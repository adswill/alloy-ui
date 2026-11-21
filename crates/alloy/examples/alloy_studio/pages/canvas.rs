//! Canvas: a node graph, free-hand drawing, paths and curves, a 3D viewport and a media player.
use crate::ui::*;
use crate::Studio;
use alloy_ui::i18n;
use alloy_ui::prelude::*;
use alloy_ui::{rgb, Frame, GraphAction, Mesh, Orbit, Path, Playback, Pointer, TestCard};

fn graph_action(s: &mut Studio, a: GraphAction) {
    match a {
        GraphAction::Move { id, pos } => {
            if let Some(n) = s.nodes.iter_mut().find(|n| n.id == id) {
                n.pos = pos;
            }
        }
        GraphAction::Select(id) => s.selected_node = id,
        GraphAction::Connect(w) => {
            s.wires.retain(|x| x.to != w.to);
            s.wires.push(w);
        }
        GraphAction::Disconnect(w) => s.wires.retain(|x| *x != w),
        GraphAction::Delete(id) => {
            s.nodes.retain(|n| n.id != id);
            s.wires.retain(|w| w.from.0 != id && w.to.0 != id);
            s.selected_node = None;
        }
    }
}

fn spiro(f: &mut Frame) {
    let p = f.palette();
    let c = pt(f.size().width / 2.0, f.size().height / 2.0);
    let mut path = Path::new();
    for i in 0..=720 {
        let t = i as f32 / 720.0 * std::f32::consts::TAU * 7.0;
        let r = 58.0;
        let (k, l) = (0.62, 0.78);
        let x = r * ((1.0 - k) * t.cos() + l * k * (t * (1.0 - k) / k).cos());
        let y = r * ((1.0 - k) * t.sin() - l * k * (t * (1.0 - k) / k).sin());
        path = if i == 0 {
            path.move_to(c.x + x, c.y + y)
        } else {
            path.line_to(c.x + x, c.y + y)
        };
    }
    f.stroke(&path, 1.4, p.accent);
    let blob = Path::new()
        .move_to(24.0, 150.0)
        .cubic_to(24.0, 100.0, 90.0, 110.0, 110.0, 150.0)
        .cubic_to(130.0, 190.0, 40.0, 200.0, 24.0, 150.0)
        .close();
    f.fill(&blob, p.accent.with_alpha(0.16));
    f.stroke(&blob, 1.5, p.accent);
    f.text(
        pt(24.0, 14.0),
        "Paths: fill, stroke, curves",
        12.0,
        p.text_secondary,
    );
}

pub fn view(s: &Studio) -> N {
    let dots = s.doodle.clone();
    let mesh = |m: Mesh, c: u32, x: f32, y: f32| object(m).color(rgb(c)).at(x, y, 0.0);
    let body: N = match s.canvas_tab {
        0 => col![
            caption("Drag headers to move, drag from an output port to an input to connect, click a wire and press Delete. Scroll to zoom, drag the background to pan.").dim(),
            node_graph(&s.nodes, &s.wires)
                .selected(s.selected_node)
                .act(graph_action),
        ]
        .gap(10.0)
        .into(),
        1 => scroll(col![
            row![
                col![caption("Canvas"), canvas(300.0, 230.0, spiro)].gap(10.0).card().pad(16.0),
                col![
                    row![
                        caption("Click and drag to draw").fill_w(),
                        button("Clear").ghost().act(|s: &mut Studio| s.doodle.clear()),
                    ]
                    .align_center(),
                    canvas::<Studio>(300.0, 230.0, move |f| {
                        let p = f.palette();
                        for w in dots.windows(2) {
                            if w[0].0 >= 0.0 && w[1].0 >= 0.0 {
                                f.line(pt(w[0].0, w[0].1), pt(w[1].0, w[1].1), 2.5, p.accent);
                            }
                        }
                        if dots.is_empty() {
                            f.text(pt(86.0, 108.0), "Draw here", 14.0, p.text_secondary);
                        }
                    })
                    .act(|s: &mut Studio, ptr| match ptr {
                        Pointer::Down(p) => s.doodle.push((p.x, p.y)),
                        Pointer::Move(p) if s.doodle.last().is_some_and(|l| l.0 >= 0.0) => {
                            s.doodle.push((p.x, p.y))
                        }
                        Pointer::Up(_) => s.doodle.push((-1.0, -1.0)),
                        _ => {}
                    }),
                ]
                .gap(10.0)
                .card()
                .pad(16.0),
            ]
            .gap(16.0),
            col![
                caption("3D viewport: drag to orbit, scroll to zoom"),
                viewport3d([
                    mesh(Mesh::torus(1.5, 0.52, 40), 0x3558A8, 0.0, 0.62),
                    mesh(Mesh::sphere(0.9, 32), 0xC2603B, 0.0, 0.62),
                    mesh(Mesh::cube(1.1), 0x4F8A6B, 3.1, 0.55),
                    mesh(Mesh::pyramid(1.4, 1.5), 0x9A6BB0, -3.1, 0.75),
                ])
                .orbit(Orbit { yaw: 0.5, pitch: 0.5, distance: 9.5 })
                .height(300.0),
            ]
            .gap(10.0)
            .card()
            .pad(16.0)
            .fill_w(),
        ]
        .gap(16.0))
        .fill_h()
        .into(),
        _ => {
            let video = TestCard::video(90);
            let audio = TestCard::audio(212);
            col![
                row![
                    media(video, s.play)
                        .title("Test card")
                        .height(320.0)
                        .act(|s: &mut Studio, a| {
                            s.play.apply(a, alloy_ui::ui().now(), 90_000);
                        }),
                    media(audio, Playback::new()).title("Level meter").height(320.0),
                ]
                .gap(16.0),
                caption("Alloy ships the player. A decoder crate implements MediaSource to feed it real frames.").dim(),
            ]
            .gap(14.0)
            .into()
        }
    };
    fixed(col![
        header(
            i18n::translate("cv.title", &[]),
            i18n::translate("cv.sub", &[]),
            text("")
        ),
        tab_bar(["Node graph", "Canvas and 3D", "Media"], at!(s.canvas_tab)),
        body.fill_h(),
    ])
}
