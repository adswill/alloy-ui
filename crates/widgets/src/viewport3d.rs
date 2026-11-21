//! A 3D viewport: lit meshes on an orbit camera, drawn with the 2D renderer.
//!
//! This is a software pipeline (perspective projection, back-face culling,
//! painter's-algorithm depth sort, flat lighting) that emits anti-aliased
//! [`DrawCommand::Polygon`]s, so it runs on every back end and in headless
//! tests with identical output. It suits product previews, CAD-style
//! gizmos and data visualisation, not a game scene. For an app that renders
//! its own GPU texture, [`Canvas`](crate::Canvas) can place a registered image
//! with `Frame::image`; sharing a live wgpu texture with the window renderer is
//! not implemented yet.

use crate::util;
use crate::{a11y::Role, ui::Ui};
use alloy_ui_core::{
    style::Style, Border, Color, DrawCommand, DrawList, Element, Event, Layout, MouseButton, Point,
    Size, Widget,
};
use alloy_ui_layout::Length;
use std::{
    f32::consts::{PI, TAU},
    rc::Rc,
};

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt().max(1e-9);
    [a[0] / l, a[1] / l, a[2] / l]
}

/// A triangle mesh with optional smooth vertex normals.
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    verts: Vec<V3>,
    normals: Vec<V3>,
    tris: Vec<[usize; 3]>,
}

impl Mesh {
    /// A mesh from raw vertices and counter-clockwise triangles (seen from
    /// outside). Normals are taken from the faces.
    pub fn new(verts: Vec<[f32; 3]>, tris: Vec<[usize; 3]>) -> Self {
        Self {
            verts,
            normals: Vec::new(),
            tris,
        }
    }
    /// An axis-aligned cube of edge `size`, centred on the origin.
    pub fn cube(size: f32) -> Self {
        let h = size / 2.0;
        let v = vec![
            [-h, -h, -h],
            [h, -h, -h],
            [h, h, -h],
            [-h, h, -h],
            [-h, -h, h],
            [h, -h, h],
            [h, h, h],
            [-h, h, h],
        ];
        let t = vec![
            [4, 5, 6],
            [4, 6, 7], // +z
            [1, 0, 3],
            [1, 3, 2], // -z
            [5, 1, 2],
            [5, 2, 6], // +x
            [0, 4, 7],
            [0, 7, 3], // -x
            [7, 6, 2],
            [7, 2, 3], // +y
            [0, 1, 5],
            [0, 5, 4], // -y
        ];
        Self::new(v, t)
    }
    /// A smooth UV sphere.
    pub fn sphere(radius: f32, segments: usize) -> Self {
        let (w, h) = (segments.max(8), (segments / 2).max(4));
        let mut verts = Vec::new();
        let mut normals = Vec::new();
        for j in 0..=h {
            let phi = PI * j as f32 / h as f32;
            for i in 0..=w {
                let th = TAU * i as f32 / w as f32;
                let n = [phi.sin() * th.cos(), phi.cos(), phi.sin() * th.sin()];
                normals.push(n);
                verts.push([n[0] * radius, n[1] * radius, n[2] * radius]);
            }
        }
        let mut tris = Vec::new();
        for j in 0..h {
            for i in 0..w {
                let a = j * (w + 1) + i;
                let b = a + w + 1;
                tris.push([a, a + 1, b]);
                tris.push([a + 1, b + 1, b]);
            }
        }
        Self {
            verts,
            normals,
            tris,
        }
    }
    /// A smooth torus lying in the XZ plane: `major` is the ring radius,
    /// `minor` the tube radius.
    pub fn torus(major: f32, minor: f32, segments: usize) -> Self {
        let (u, v) = (segments.max(12), (segments / 2).max(8));
        let mut verts = Vec::new();
        let mut normals = Vec::new();
        for i in 0..=u {
            let a = TAU * i as f32 / u as f32;
            for j in 0..=v {
                let b = TAU * j as f32 / v as f32;
                let n = [b.cos() * a.cos(), b.sin(), b.cos() * a.sin()];
                normals.push(n);
                verts.push([
                    (major + minor * b.cos()) * a.cos(),
                    minor * b.sin(),
                    (major + minor * b.cos()) * a.sin(),
                ]);
            }
        }
        let mut tris = Vec::new();
        for i in 0..u {
            for j in 0..v {
                let a = i * (v + 1) + j;
                let b = a + v + 1;
                tris.push([a, a + 1, b]);
                tris.push([a + 1, b + 1, b]);
            }
        }
        Self {
            verts,
            normals,
            tris,
        }
    }
    /// A four-sided pyramid with its base on `y = -height / 2`.
    pub fn pyramid(base: f32, height: f32) -> Self {
        let (h, y) = (base / 2.0, height / 2.0);
        Self::new(
            vec![
                [-h, -y, -h],
                [h, -y, -h],
                [h, -y, h],
                [-h, -y, h],
                [0.0, y, 0.0],
            ],
            vec![
                [1, 0, 4],
                [2, 1, 4],
                [3, 2, 4],
                [0, 3, 4],
                [0, 2, 3],
                [0, 1, 2],
            ],
        )
    }
    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.tris.len()
    }
}

/// A mesh placed in the scene.
#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    mesh: Rc<Mesh>,
    color: Color,
    position: V3,
    rotation: V3,
    scale: f32,
    wireframe: bool,
}

impl Object {
    /// `mesh` at the origin in the accent colour.
    pub fn new(mesh: Mesh) -> Self {
        Self {
            mesh: Rc::new(mesh),
            color: Color::hex(0x4A6FA5),
            position: [0.0; 3],
            rotation: [0.0; 3],
            scale: 1.0,
            wireframe: false,
        }
    }
    /// Surface colour.
    pub fn color(mut self, c: Color) -> Self {
        self.color = c;
        self
    }
    /// Move to `(x, y, z)`; y is up.
    pub fn at(mut self, x: f32, y: f32, z: f32) -> Self {
        self.position = [x, y, z];
        self
    }
    /// Rotate by Euler angles in radians (applied Y, then X, then Z).
    pub fn rotate(mut self, rx: f32, ry: f32, rz: f32) -> Self {
        self.rotation = [rx, ry, rz];
        self
    }
    /// Uniform scale.
    pub fn scale(mut self, s: f32) -> Self {
        self.scale = s;
        self
    }
    /// Draw edges only.
    pub fn wireframe(mut self) -> Self {
        self.wireframe = true;
        self
    }
    fn world(&self, v: V3) -> V3 {
        let [rx, ry, rz] = self.rotation;
        let mut p = [v[0] * self.scale, v[1] * self.scale, v[2] * self.scale];
        p = rot_y(p, ry);
        p = rot_x(p, rx);
        p = rot_z(p, rz);
        [
            p[0] + self.position[0],
            p[1] + self.position[1],
            p[2] + self.position[2],
        ]
    }
    fn world_dir(&self, v: V3) -> V3 {
        let [rx, ry, rz] = self.rotation;
        rot_z(rot_x(rot_y(v, ry), rx), rz)
    }
}

fn rot_x(p: V3, a: f32) -> V3 {
    let (s, c) = a.sin_cos();
    [p[0], p[1] * c - p[2] * s, p[1] * s + p[2] * c]
}
fn rot_y(p: V3, a: f32) -> V3 {
    let (s, c) = a.sin_cos();
    [p[0] * c + p[2] * s, p[1], -p[0] * s + p[2] * c]
}
fn rot_z(p: V3, a: f32) -> V3 {
    let (s, c) = a.sin_cos();
    [p[0] * c - p[1] * s, p[0] * s + p[1] * c, p[2]]
}

/// Orbit camera pose, readable for tests and tooling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orbit {
    /// Rotation around the vertical axis, radians.
    pub yaw: f32,
    /// Elevation above the horizon, radians (clamped to just under ±90°).
    pub pitch: f32,
    /// Distance from the target.
    pub distance: f32,
}

impl Default for Orbit {
    fn default() -> Self {
        Self {
            yaw: 0.6,
            pitch: 0.42,
            distance: 7.0,
        }
    }
}

/// Lit meshes on an orbit camera. Drag to orbit, scroll to zoom. The view
/// state (yaw, pitch, distance) lives in the [`Ui`]; set the starting pose
/// with [`Viewport3d::orbit`]. Stays idle unless [`Viewport3d::spin`] is set.
pub struct Viewport3d {
    ui: Ui,
    id: u64,
    objects: Vec<Object>,
    initial: Orbit,
    height: Length,
    spin: f32,
    grid: bool,
}

const K_YAW: u64 = 0x3D00_0001;
const K_PITCH: u64 = 0x3D00_0002;
const K_DIST: u64 = 0x3D00_0003;
const K_DRAG: u64 = 0x3D00_0004;
const K_LX: u64 = 0x3D00_0005;
const K_LY: u64 = 0x3D00_0006;

impl Viewport3d {
    /// A viewport showing `objects`. `id` keeps the camera across rebuilds.
    pub fn new(ui: &Ui, id: u64, objects: impl Into<Vec<Object>>) -> Self {
        Self {
            ui: ui.clone(),
            id,
            objects: objects.into(),
            initial: Orbit::default(),
            height: Length::Fixed(280.0),
            spin: 0.0,
            grid: true,
        }
    }
    /// Starting camera pose (used until the user orbits).
    pub fn orbit(mut self, o: Orbit) -> Self {
        self.initial = o;
        self
    }
    /// Turntable: rotate the camera `radians_per_second` around the target.
    /// This keeps the app redrawing; leave it off for a static preview.
    pub fn spin(mut self, radians_per_second: f32) -> Self {
        self.spin = radians_per_second;
        self
    }
    /// Show or hide the floor grid (default shown).
    pub fn grid(mut self, on: bool) -> Self {
        self.grid = on;
        self
    }
    /// Height (default 280 px).
    pub fn height(mut self, h: Length) -> Self {
        self.height = h;
        self
    }
    /// Box as an element.
    pub fn into_element<M: 'static>(self) -> Element<M> {
        Element::new(self)
    }

    fn slot(&self, k: u64) -> Option<i64> {
        self.ui.slot(self.id ^ k)
    }
    /// The camera as currently shown (before any turntable spin).
    pub fn pose(&self) -> Orbit {
        Orbit {
            yaw: self
                .slot(K_YAW)
                .map_or(self.initial.yaw, |v| v as f32 / 1000.0),
            pitch: self
                .slot(K_PITCH)
                .map_or(self.initial.pitch, |v| v as f32 / 1000.0),
            distance: self
                .slot(K_DIST)
                .map_or(self.initial.distance, |v| v as f32 / 1000.0),
        }
    }
    fn set_pose(&self, o: Orbit) {
        self.ui
            .set_slot(self.id ^ K_YAW, Some((o.yaw * 1000.0) as i64));
        self.ui
            .set_slot(self.id ^ K_PITCH, Some((o.pitch * 1000.0) as i64));
        self.ui
            .set_slot(self.id ^ K_DIST, Some((o.distance * 1000.0) as i64));
    }
}

impl<M> Widget<M> for Viewport3d {
    fn style(&self) -> Style {
        util::sized(Length::Fill(1), self.height)
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(
            240.0,
            match self.height {
                Length::Fixed(h) => h,
                _ => 240.0,
            },
        )
    }
    fn animating(&self) -> bool {
        self.spin != 0.0 && !self.ui.reduced_motion()
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        _out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        match event {
            Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                self.ui.set_slot(self.id ^ K_DRAG, Some(1));
                self.ui
                    .set_slot(self.id ^ K_LX, Some((cursor.x * 16.0) as i64));
                self.ui
                    .set_slot(self.id ^ K_LY, Some((cursor.y * 16.0) as i64));
                true
            }
            Event::CursorMoved(p) if self.slot(K_DRAG).is_some() => {
                let (lx, ly) = (
                    self.slot(K_LX).unwrap_or(0) as f32 / 16.0,
                    self.slot(K_LY).unwrap_or(0) as f32 / 16.0,
                );
                let mut o = self.pose();
                o.yaw -= (p.x - lx) * 0.01;
                o.pitch = (o.pitch + (p.y - ly) * 0.01).clamp(-1.45, 1.45);
                self.set_pose(o);
                self.ui.set_slot(self.id ^ K_LX, Some((p.x * 16.0) as i64));
                self.ui.set_slot(self.id ^ K_LY, Some((p.y * 16.0) as i64));
                true
            }
            Event::MouseReleased(MouseButton::Left) if self.slot(K_DRAG).is_some() => {
                self.ui.set_slot(self.id ^ K_DRAG, None);
                true
            }
            Event::Scrolled { dy, .. } if b.contains(cursor) => {
                let mut o = self.pose();
                let d = (o.distance * (1.0 - dy * 0.002)).clamp(2.0, 40.0);
                if (d - o.distance).abs() < 1e-4 {
                    return false;
                }
                o.distance = d;
                self.set_pose(o);
                true
            }
            _ => false,
        }
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let b = layout.bounds;
        self.ui.describe(
            Role::Label,
            &format!("3D viewport, {} objects", self.objects.len()),
            &b,
            |_| {},
        );
        out.push(DrawCommand::Quad {
            bounds: b,
            fill: p.surface,
            radius: t.radii.large,
            border: Some(Border {
                color: p.border,
                width: 1.0,
            }),
            shadow: None,
        });
        out.push(DrawCommand::PushClip(b));

        let mut o = self.pose();
        if self.spin != 0.0 && !self.ui.reduced_motion() {
            o.yaw += self.spin * self.ui.now() as f32 / 1000.0;
        }
        // Camera basis (y up, looking at the origin).
        let eye = [
            o.distance * o.pitch.cos() * o.yaw.sin(),
            o.distance * o.pitch.sin(),
            o.distance * o.pitch.cos() * o.yaw.cos(),
        ];
        let fwd = norm(sub([0.0, 0.0, 0.0], eye));
        let right = norm(cross(fwd, [0.0, 1.0, 0.0]));
        let up = cross(right, fwd);
        let center = Point::new(
            b.origin.x + b.size.width / 2.0,
            b.origin.y + b.size.height / 2.0,
        );
        let focal = b.size.height.min(b.size.width * 0.9) * 1.1;
        // View-space point and its screen projection.
        let view = |w: V3| {
            let d = sub(w, eye);
            [dot(d, right), dot(d, up), dot(d, fwd)]
        };
        let project = |v: V3| {
            Point::new(
                center.x + v[0] / v[2] * focal,
                center.y - v[1] / v[2] * focal,
            )
        };

        // Floor grid, fading toward its edge.
        if self.grid {
            let n: i32 = 6;
            for i in -n..=n {
                let fade = 1.0 - (i.abs() as f32 / (n as f32 + 1.0)).powi(2);
                for axis in 0..2 {
                    let (a, z) = if axis == 0 {
                        ([i as f32, 0.0, -(n as f32)], [i as f32, 0.0, n as f32])
                    } else {
                        ([-(n as f32), 0.0, i as f32], [n as f32, 0.0, i as f32])
                    };
                    let (va, vz) = (view(a), view(z));
                    if va[2] < 0.2 || vz[2] < 0.2 {
                        continue;
                    }
                    out.push(DrawCommand::Line {
                        from: project(va),
                        to: project(vz),
                        width: 1.0,
                        color: p
                            .border_strong
                            .with_alpha(0.35 * fade + if i == 0 { 0.15 } else { 0.0 }),
                    });
                }
            }
        }

        // Collect lit faces from every object, then paint far to near.
        struct Face {
            depth: f32,
            pts: [Point; 3],
            color: Color,
        }
        let mut faces: Vec<Face> = Vec::new();
        let light = norm([-0.45, 0.6, -0.65]); // view space, toward the light: left, up, camera-side
        for obj in &self.objects {
            let m = &obj.mesh;
            let vs: Vec<V3> = m.verts.iter().map(|v| view(obj.world(*v))).collect();
            if obj.wireframe {
                let mut seen = std::collections::HashSet::new();
                for tri in &m.tris {
                    for k in 0..3 {
                        let (a, z) = (tri[k], tri[(k + 1) % 3]);
                        if !seen.insert((a.min(z), a.max(z))) || vs[a][2] < 0.2 || vs[z][2] < 0.2 {
                            continue;
                        }
                        out.push(DrawCommand::Line {
                            from: project(vs[a]),
                            to: project(vs[z]),
                            width: 1.0,
                            color: obj.color,
                        });
                    }
                }
                continue;
            }
            for tri in &m.tris {
                let (a, bb, c) = (vs[tri[0]], vs[tri[1]], vs[tri[2]]);
                if a[2] < 0.2 || bb[2] < 0.2 || c[2] < 0.2 {
                    continue;
                }
                let n = norm(cross(sub(bb, a), sub(c, a)));
                let centroid = [
                    (a[0] + bb[0] + c[0]) / 3.0,
                    (a[1] + bb[1] + c[1]) / 3.0,
                    (a[2] + bb[2] + c[2]) / 3.0,
                ];
                // Cull faces turned away from the eye. View space mirrors the world's
                // handedness (depth grows away from the camera), so a front-facing
                // counter-clockwise triangle has `n` pointing away from the eye.
                if dot(n, centroid) <= 0.0 {
                    continue;
                }
                // Smooth normals when the mesh has them, else the face normal.
                let shade_n = if m.normals.len() == m.verts.len() {
                    let sum = [0, 1, 2].map(|k| {
                        let wn = obj.world_dir(m.normals[tri[k]]);
                        [dot(wn, right), dot(wn, up), dot(wn, fwd)]
                    });
                    norm([
                        sum[0][0] + sum[1][0] + sum[2][0],
                        sum[0][1] + sum[1][1] + sum[2][1],
                        sum[0][2] + sum[1][2] + sum[2][2],
                    ])
                } else {
                    [-n[0], -n[1], -n[2]]
                };
                let lambert = dot(shade_n, light).max(0.0);
                let k = 0.34 + 0.66 * lambert;
                let color = Color {
                    r: (obj.color.r * k).min(1.0),
                    g: (obj.color.g * k).min(1.0),
                    b: (obj.color.b * k).min(1.0),
                    a: obj.color.a,
                };
                faces.push(Face {
                    depth: centroid[2],
                    pts: [project(a), project(bb), project(c)],
                    color,
                });
            }
        }
        faces.sort_by(|x, y| y.depth.total_cmp(&x.depth));
        for f in faces {
            // Grow each face by a hair so neighbours overlap instead of
            // leaving anti-aliasing seams of background between them.
            let cx = (f.pts[0].x + f.pts[1].x + f.pts[2].x) / 3.0;
            let cy = (f.pts[0].y + f.pts[1].y + f.pts[2].y) / 3.0;
            let points = f
                .pts
                .iter()
                .map(|q| {
                    let (dx, dy) = (q.x - cx, q.y - cy);
                    let l = dx.hypot(dy).max(1e-3);
                    Point::new(q.x + dx / l * 0.45, q.y + dy / l * 0.45)
                })
                .collect();
            out.push(DrawCommand::Polygon {
                points,
                fill: f.color,
            });
        }
        out.push(DrawCommand::PopClip);
    }
}
