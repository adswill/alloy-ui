//! System: everything that talks to the operating system. Each card says what it uses and how
//! to try it. Features a platform cannot do are logged and skipped, never fatal.
use crate::ui::*;
use crate::Studio;
use alloy_ui::prelude::*;
use alloy_ui::{clipboard, i18n};

pub fn export_csv(s: &mut Studio) {
    match files::save("orders.csv").title("Export orders").pick() {
        Some(path) => match std::fs::write(&path, s.orders_csv()) {
            Ok(()) => {
                s.log(format!("Exported {}", path.display()));
                toast("Exported 5 000 orders");
            }
            Err(e) => {
                toast(format!("Could not write the file: {e}")).error();
            }
        },
        None => {
            toast("Export cancelled");
        }
    }
}

pub fn open_file(s: &mut Studio) {
    if let Some(p) = files::open()
        .title("Pick any file")
        .filter("Images and text", &["png", "jpg", "txt", "md"])
        .pick()
    {
        s.picked_file = p.display().to_string();
        s.log(format!("Picked {}", p.display()));
        toast("File chosen");
    }
}

pub fn notify_now(s: &mut Studio) {
    notify("Alloy Studio", "This is a native notification.");
    s.log("Sent a notification");
}

/// A 64x64 gradient, as raw RGBA, to put on the clipboard as a picture.
fn sample_image() -> Vec<u8> {
    let mut px = Vec::with_capacity(64 * 64 * 4);
    for y in 0..64u32 {
        for x in 0..64u32 {
            px.extend([(x * 4) as u8, (y * 4) as u8, 200, 255]);
        }
    }
    px
}

fn card(title: &str, blurb: &str, body: impl Into<N>) -> N {
    col![
        text(title.to_string()).label(),
        caption(blurb.to_string()).dim(),
        body.into(),
    ]
    .gap(8.0)
    .card()
    .pad(16.0)
    .fill_w()
    .into()
}

pub fn view(s: &Studio) -> N {
    let sys = system();
    let yes = |b: bool| if b { "on" } else { "off" };
    page(col![
        header(
            i18n::translate("sy.title", &[]),
            i18n::translate("sy.sub", &[]),
            text("")
        ),
        row![
            card(
                "Notifications",
                "Notification Center, a Windows toast or the freedesktop service.",
                button("Send a notification").act(notify_now),
            ),
            card(
                "File dialogs",
                "NSOpenPanel, the common item dialog, zenity or kdialog.",
                col![
                    row![
                        button("Open a file").act(|s: &mut Studio| open_file(s)),
                        button("Export CSV").act(|s: &mut Studio| export_csv(s)),
                        button("Choose a folder").act(|s: &mut Studio| {
                            if let Some(p) = files::folder().title("Pick a folder").pick() {
                                s.log(format!("Folder {}", p.display()));
                                s.picked_file = p.display().to_string();
                            }
                        }),
                    ]
                    .gap(8.0),
                    caption(if s.picked_file.is_empty() {
                        "Nothing chosen yet".to_string()
                    } else {
                        s.picked_file.clone()
                    })
                    .dim(),
                ]
                .gap(8.0),
            ),
        ]
        .gap(14.0),
        row![
            card(
                "Clipboard",
                "Plain text, HTML, an image and file references, with several flavours at once.",
                col![
                    row![
                        button("Copy text").act(|s: &mut Studio| {
                            clipboard::copy("Hello from Alloy Studio");
                            s.log("Copied text");
                        }),
                        button("Copy HTML table").act(|s: &mut Studio| {
                            clipboard::copy_html(
                                "<table><tr><th>Region</th><th>Orders</th></tr><tr><td>EMEA</td><td>33 412</td></tr></table>",
                                "Region\tOrders\nEMEA\t33 412",
                            );
                            s.log("Copied an HTML table");
                        }),
                        button("Copy image").act(|s: &mut Studio| {
                            clipboard::copy_image(64, 64, &sample_image());
                            s.log("Copied a 64x64 image");
                        }),
                    ]
                    .gap(8.0),
                    row![
                        button("Paste").primary().act(|s: &mut Studio| {
                            let c = clipboard::paste();
                            s.clip_note = format!(
                                "text: {}, html: {}, image: {}, files: {}",
                                c.text.as_deref().map(|t| t.chars().take(28).collect::<String>()).unwrap_or_else(|| "none".into()),
                                c.html.is_some(),
                                c.image.is_some(),
                                c.files.len()
                            );
                        }),
                        caption(&s.clip_note).dim(),
                    ]
                    .gap(10.0)
                    .align_center(),
                ]
                .gap(8.0),
            ),
            card(
                "Windows",
                "One state, several windows. Edit anything and both windows follow.",
                row![
                    button(if s.monitor { "Close the monitor" } else { "Open the monitor window" })
                        .primary()
                        .act(|s: &mut Studio| s.monitor = !s.monitor),
                    caption("Cmd/Ctrl+N does the same.").dim(),
                ]
                .gap(10.0)
                .align_center(),
            ),
        ]
        .gap(14.0),
        card(
            "Drop files here",
            "Drag files from Finder, Explorer or your file manager onto the window.",
            col![
                text(if files_hovering() {
                    "Drop to add them".to_string()
                } else if s.dropped.is_empty() {
                    "Waiting for files".to_string()
                } else {
                    format!("{} received", s.dropped.len())
                })
                .accent_if(files_hovering()),
                col(s.dropped.iter().rev().take(4).map(|f| caption(f.clone()).dim())).gap(2.0),
            ]
            .gap(6.0)
            .pad(14.0)
            .h(110.0)
            .card()
            .fill_w(),
        ),
        row![
            card(
                "Menus, tray and hotkeys",
                "A real menu bar (macOS, Windows), a tray icon, and Ctrl+Alt+S anywhere on the desktop.",
                col![
                    caption("Menu: File, View, Help. Every item is also in the palette (Cmd/Ctrl+K)."),
                    caption("Tray: click for a toast; right-click for the menu."),
                    caption("Hotkey: Ctrl+Alt+S fires even while another app is in front."),
                ]
                .gap(4.0),
            ),
            card(
                "Deep links and one copy",
                "alloy-studio://... opens this window; a second launch hands over to the first.",
                col![
                    caption(if s.last_url.is_empty() {
                        "No link received yet".to_string()
                    } else {
                        format!("Last link: {}", s.last_url)
                    })
                    .dim(),
                    caption("Try: open alloy-studio://hello (macOS needs a bundle; see RUN.md)").dim(),
                ]
                .gap(4.0),
            ),
        ]
        .gap(14.0),
        row![
            card(
                "What the OS says",
                "Read at start and when it changes. Reduced motion is always followed.",
                col![
                    caption(format!("Dark mode: {}", yes(sys.dark))),
                    caption(format!("Reduced motion: {}", yes(sys.reduced_motion))),
                    caption(format!("High contrast: {}", yes(sys.high_contrast))),
                    caption(match sys.accent {
                        Some([r, g, b]) => format!("Accent: #{r:02X}{g:02X}{b:02X}"),
                        None => "Accent: not reported".into(),
                    }),
                ]
                .gap(4.0),
            ),
            card(
                "Activity",
                "Everything on this page logs here.",
                col(if s.activity.is_empty() {
                    vec![Node::from(caption("Nothing yet").dim())]
                } else {
                    s.activity.iter().map(|a| Node::from(caption(a.clone()))).collect()
                })
                .gap(4.0),
            ),
        ]
        .gap(14.0),
    ])
}

/// The second window: live charts and a mirror of the shared state.
pub fn monitor(s: &Studio) -> N {
    col![
        heading("Monitor"),
        text("A second window over the same state.").dim(),
        stream([("CPU", &s.cpu), ("Memory", &s.mem)])
            .unit("%")
            .window("last 60 s")
            .height(190.0),
        stats([
            (
                "Open tasks",
                s.work.tasks.iter().filter(|t| !t.done).count().to_string()
            ),
            ("Orders", s.orders.len().to_string()),
            (
                "Cards",
                s.work
                    .lanes
                    .iter()
                    .map(|l| l.cards.len())
                    .sum::<usize>()
                    .to_string()
            ),
        ]),
        row![
            check("Dark", at!(s.dark)),
            button("Close this window").act(|s: &mut Studio| s.monitor = false),
        ]
        .gap(16.0)
        .align_center(),
    ]
    .gap(14.0)
    .pad(20.0)
    .into()
}
