# Design brief (identical for Alloy, iced and Tauri)

Three desktop apps, each opening a 960x640 window. Same behaviour, same data, same look.

**Look.** Calm and precise, no gradients, no emoji, no decorative icons. Warm neutral surfaces, one indigo accent (#4f6bed), 8 px corner radius on cards and buttons, hairline 1 px borders, 14 px body text, 12 px captions, headings 20 px semibold. Sidebar slightly darker than the page. Light theme by default; each app has a Dark toggle (Settings only needs a working one; in the other two apps the toggle is optional).

## 1. Notes
Left: search box plus list of notes (title, first line preview, pinned notes first with a small "Pinned" caption). Right: title field, multi-line body, word count, Pin and Delete buttons. Seed with five notes. Typing in the search filters live. Selecting a note edits it. New note button at the top of the list.

## 2. Pulse (dashboard)
Header "Pulse" with a segmented range control: 7 days / 30 days / 90 days. Four stat tiles (Revenue, Active users, Conversion, p95 latency) with a value and a delta. A bar chart of revenue per interval (7, 10 and 12 bars for the three ranges). A table of recent orders: Customer, Plan, Amount, Status (Paid, Pending, Refunded; Refunded in danger red). Switching the range changes tiles and chart.

## 3. Preferences
Sidebar: Appearance, Notifications, Privacy, About. Appearance: Light/Dark segmented control (applies live), accent colour choice of four swatches, text size slider 12 to 20 with a live preview sentence. Notifications: three toggles, volume slider, delivery dropdown (Instantly, Hourly digest, Daily digest). Privacy: two toggles and a "Clear history" button that shows a confirmation. About: name, version, a one-line description.
