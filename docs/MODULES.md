# Ready-made modules

Whole interface pieces that look finished with one call. Everything is in `alloy_ui::prelude::*`.
Renders: `gallery/modules/*.png` in the project files; try them with
`cargo run -p alloy-ui --example modules -- --headless out.png --click Feeds [--dark]`.

| Module | One line |
| --- | --- |
| KPI card | `kpi("Revenue", "$48.2k").delta(12.4).hint("vs last week").trend(series(24))` |
| Gauge, radial | `gauge(0.78).label("Memory").bands()`, `radial(0.64).label("Uploaded")` |
| Avatars | `avatar("Mira Okafor").presence(Presence::Online)`, `avatar_group(names).max(4)` |
| Timeline | `timeline([moment("12:04", "Deploy finished").detail("Live").success()])` |
| Changelog | `changelog([release("0.4.0", "Nov 21").latest().added([..]).fixed([..])])` |
| Activity feed | `activity_feed([activity("Mira", "merged", "Add gauges", "4 min")])` |
| Notification centre | `notification_center(items).on_read(\|s, id\| ..).on_read_all(\|s\| ..)` |
| Empty state | `empty_state(Art::Inbox, "All caught up", "Nothing new.").action(button("Compose"))` |
| Breadcrumb | `breadcrumb(["Projects", "Alloy"]).act(\|s, i\| ..)` |
| Page header | `page_header("Deployments").subtitle(..).crumbs(..).badge(..).actions([..])` |
| Step tracker | `tracker(["Cart", "Shipping", "Payment"], 1)` |
| Settings page | `settings_page("Settings").section("Sync", [setting("Wi-Fi only", "..", toggle(..))])` |
| Sign in / up | `login("Alloy").providers(["GitHub"]).on_submit(\|s, email, pw\| ..)`, `.create_account()` |
| Chat | `chat(msgs)` with `message(who, text, when)` / `reply(text, when)`; `composer().on_send(\|s, text\| ..)` |
| Plan picker | `plan_picker([plan("Team", 12.0).features([..]).recommended()]).on_choose(\|s, i\| ..)` |
| File browser | `file_browser([folder(..), file(..)]).path([..]).on_open(\|s, name\| ..)` |
| Gallery grid | `gallery_grid([tile("Dusk", "2 MB")]).cols(4)` |
| Spotlight tour | `let t = tour(3); t.spot(0, button("New"), "Start here", "..")` |
| Command bar | `command_bar([hit("Actions", "New project").keys("Ctrl+N")]).on_pick(\|s, i\| ..)` |
| Toast tones | `toast("Saved").success()`, `.warning()`, `.info()` (and the existing `.error()`) |

Two new widgets back them: `Tap` (any element clickable, hover overlay, selected tint) and
`Spotlight` (dim the window around one widget with a card beside it; modal, Escape dismisses).

## Rules the modules follow

- Static modules are generic over the message type; interactive ones take closures over your state.
  Their own state (query, selection, billing period, tour step, draft) lives in the view.
- No side stripes, no hero metrics, no glow: status is a dot plus a word, accent is used once.
- Entrances use the existing animation layer (gauge arcs ease to new values, the tour dims in and out).
- Golden tests (`crates/alloy/tests/modules.rs`) render shapes only, light and dark, so they hold on
  every OS; behaviour is tested through the real runtime.
