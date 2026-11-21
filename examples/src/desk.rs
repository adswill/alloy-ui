//! Desk: the system integration in one small app. Drag cards between lists, drop files on the
//! window, open `alloy-desk://` links (a second launch reaches the running copy), copy rich
//! content, and sit on a see-through window that follows the OS accent and reduced motion.
use alloy_ui::{clipboard, prelude::*};
use std::path::PathBuf;

/// Window size in logical pixels.
pub const SIZE: (f32, f32) = (760.0, 480.0);

/// The state.
#[derive(Clone, Default)]
pub struct Desk {
    todo: Vec<String>,
    done: Vec<String>,
    files: Vec<PathBuf>,
    link: String,
    pasted: String,
}

fn list(title: &str, items: &[String]) -> Node<Act<Desk>> {
    let cards: Vec<Node<Act<Desk>>> = items.iter().map(|c| text(c).pad(4.0).card().fill_w().drag(c.clone())).collect();
    col![heading(title), col(cards).gap(8.0)].gap(12.0).pad(14.0).w(230.0).h(320.0).surface().round(12.0).into()
}

/// The app.
pub fn app() -> impl App {
    let seed = Desk {
        todo: ["Write release notes", "Review the PR", "Book flights"].map(String::from).to_vec(),
        done: vec!["Fix the crash".into()],
        ..Desk::default()
    };
    sketch(seed, |d: &Desk| {
        let drop_hint = if files_hovering() { "Drop to add".to_string() } else { format!("{} files dropped", d.files.len()) };
        col![
            row![
                list("To do", &d.todo).drop_act(|d: &mut Desk, c: String| {
                    d.done.retain(|x| *x != c);
                    if !d.todo.contains(&c) { d.todo.push(c) }
                }),
                list("Done", &d.done).drop_act(|d: &mut Desk, c: String| {
                    d.todo.retain(|x| *x != c);
                    if !d.done.contains(&c) { d.done.push(c) }
                }),
                col![
                    text(drop_hint).heading(),
                    text(if d.link.is_empty() { "No link opened".into() } else { format!("Opened {}", d.link) }).dim(),
                    text(&d.pasted).dim(),
                ].gap(8.0),
            ].gap(16.0),
            row![
                button("Copy as HTML").act(|d: &mut Desk| {
                    let li: String = d.todo.iter().map(|t| format!("<li>{t}</li>")).collect();
                    clipboard::copy_html(&format!("<h3>To do</h3><ul>{li}</ul>"), "");
                }),
                button("Copy image").act(|_| {
                    let px: Vec<u8> = (0..64u32 * 64).flat_map(|i| [(i % 64 * 4) as u8, (i / 64 * 4) as u8, 160, 255]).collect();
                    clipboard::copy_image(64, 64, &px);
                }),
                button("Paste").primary().act(|d: &mut Desk| {
                    let c = clipboard::paste();
                    d.pasted = format!("{} chars, html {}, image {}, {} files", c.text.map_or(0, |t| t.len()), c.html.is_some(), c.image.is_some(), c.files.len());
                }),
            ].gap(8.0),
        ]
        .gap(16.0)
        .pad(24.0)
        .fill()
    })
    .on_drop(|d, files| d.files.extend(files))
    .on_open_url(|d, url| d.link = url)
    .single_instance("dev.alloyui.desk")
    .url_scheme("alloy-desk")
    .window_effect(WindowEffect::Mica)
    .system_accent()
}

/// Open the window (or render headless, see the crate docs).
pub fn run() {
    app().run_sized("Desk", SIZE.0, SIZE.1)
}
