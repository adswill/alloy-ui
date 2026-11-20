//! Headless runner: drives an app through the real runtime and rasterises it.

use alloy_ui_core::runtime::Runtime;
use alloy_ui_core::{Application, DrawCommand, Event, Key, Modifiers, MouseButton, Point, Size};
use alloy_ui_render::{CpuRenderer, HeadlessRenderer, Image};

use crate::kit::layouter;

/// One scripted user action.
#[derive(Debug, Clone)]
pub enum Step {
    /// Click the widget whose text is exactly this.
    Click(&'static str),
    /// Type text into the focused editor.
    Type(&'static str),
    /// Press a key.
    Press(Key),
}

/// An app plus the script that demonstrates it.
pub struct Demo<A: Application> {
    /// Example name (also the PNG file stem).
    pub name: &'static str,
    /// Initial state.
    pub app: A,
    /// Window size in logical pixels.
    pub size: (f32, f32),
    /// Scripted input.
    pub steps: Vec<Step>,
}

/// Build the runtime for an app at `size`.
pub fn runtime<A: Application>(app: A, size: (f32, f32)) -> Runtime<A> {
    Runtime::new(app, Size::new(size.0, size.1), layouter::<A::Message>)
}

/// Find the first text command with exactly this content and return a point inside it.
pub fn find_text<A: Application>(rt: &Runtime<A>, text: &str) -> Option<Point> {
    rt.draw().0.iter().find_map(|c| match c {
        DrawCommand::Text {
            origin, content, ..
        } if content == text => Some(Point::new(origin.x + 3.0, origin.y + 3.0)),
        _ => None,
    })
}

/// Send a real click (move, press, release) at the first text matching `text`.
/// Returns false if no such text is on screen.
pub fn click_text<A: Application>(rt: &mut Runtime<A>, text: &str) -> bool {
    let Some(p) = find_text(rt, text) else {
        return false;
    };
    rt.handle_event(&Event::CursorMoved(p));
    rt.handle_event(&Event::MousePressed(MouseButton::Left));
    rt.handle_event(&Event::MouseReleased(MouseButton::Left));
    true
}

/// Apply one step. Panics if a click target is missing: a demo script that
/// does not match its UI is a bug.
pub fn apply<A: Application>(rt: &mut Runtime<A>, step: &Step) {
    match step {
        Step::Click(t) => assert!(click_text(rt, t), "no widget with text {t:?}"),
        Step::Type(s) => {
            rt.handle_event(&Event::TextInput((*s).to_string()));
        }
        Step::Press(k) => {
            rt.handle_event(&Event::KeyPressed(k.clone(), Modifiers::default()));
        }
    }
}

/// Run a demo to completion and render the final frame.
pub fn render_demo<A: Application>(demo: Demo<A>) -> (Runtime<A>, Image) {
    let mut rt = runtime(demo.app, demo.size);
    for s in &demo.steps {
        apply(&mut rt, s);
    }
    let img = render(&rt, demo.size);
    (rt, img)
}

/// Rasterise the runtime's current frame at 2x.
pub fn render<A: Application>(rt: &Runtime<A>, size: (f32, f32)) -> Image {
    let mut r = CpuRenderer::new(size.0 as u32 * 2, size.1 as u32 * 2);
    r.render_to_image(&rt.draw(), size.0 as u32 * 2, size.1 as u32 * 2, 2.0)
}

/// Entry point shared by the example binaries: run the demo, write
/// `<name>.png` into the directory given as the first argument (default `.`).
pub fn main_headless<A: Application>(demo: Demo<A>) {
    let name = demo.name;
    let t0 = std::time::Instant::now();
    let (_rt, img) = render_demo(demo);
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let path = std::path::Path::new(&dir).join(format!("{name}.png"));
    let png = img.encode_png().expect("png encode");
    std::fs::write(&path, png).expect("write png");
    println!(
        "{name}: headless run in {:.1} ms, wrote {}",
        t0.elapsed().as_secs_f64() * 1e3,
        path.display()
    );
}
