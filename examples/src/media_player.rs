//! A media player that plays a real file: `media_player clip.mp4` (video through ffmpeg, sound
//! through symphonia and the OS audio API) or any audio file. With no path it plays the built-in
//! test card, so the window always opens. Needs the `media` feature.
use alloy_ui::codecs::{open, Media};
use alloy_ui::prelude::*;
use alloy_ui::{ui, MediaAction, MediaSource, Playback, TestCard};
use std::rc::Rc;

/// Window size in logical pixels.
pub const SIZE: (f32, f32) = (760.0, 520.0);

struct Player {
    source: Rc<dyn MediaSource>,
    name: String,
    playback: Playback,
    note: Option<String>,
}

fn load() -> (Rc<dyn MediaSource>, String, Option<String>) {
    match std::env::args().nth(1).filter(|a| !a.starts_with("--")) {
        Some(path) => match open(&path) {
            Ok(m) => {
                let m: Media = m;
                (Rc::new(m), path, None)
            }
            Err(e) => (
                Rc::new(TestCard::video(60)),
                "Test card".into(),
                Some(format!("Could not open {path}: {e}")),
            ),
        },
        None => (Rc::new(TestCard::video(60)), "Test card".into(), None),
    }
}

/// The app.
pub fn app() -> impl App {
    let (source, name, note) = load();
    sketch(
        Player {
            source,
            name,
            playback: Playback::new(),
            note,
        },
        |p| {
            let dur = p.source.duration_ms();
            col![
                col![
                    heading("Media player"),
                    text(p.note.clone().unwrap_or_else(|| p.name.clone())).dim()
                ]
                .gap(2.0),
                media(p.source.clone(), p.playback)
                    .title(p.name.clone())
                    .height(380.0)
                    .act(move |p: &mut Player, a: MediaAction| {
                        p.playback.apply(a, ui().now(), dur)
                    }),
            ]
            .gap(14.0)
            .pad(20.0)
            .fill()
        },
    )
}

/// Open the window (or render headless, see the crate docs).
pub fn run() {
    app().run_sized("Media player", SIZE.0, SIZE.1)
}
