# Visual widgets

Run `cargo run -p alloy-ui --example viz` for all of these in one window. Renders live in `gallery/widgets4/`.

| Widget | One line | State lives in |
|---|---|---|
| `pie(..).donut()` | pie or ring, legend, hover lift, click to select | your app (`.selected`) |
| `candles(..)` | OHLC with volume bars and a crosshair readout | your app |
| `stream(..)` | live line chart over a `Ring`; samples glide in | a `Ring` per series |
| `kanban(&lanes)` | drag cards between and within lanes; Alt+arrows from the keyboard | your app (`mv.apply(&mut lanes)`) |
| `calendar(day, view, &entries)` | month, week, day; drag to move, drag the bottom edge to resize, click to create | your app (`CalAction`) |
| `node_graph(&nodes, &wires)` | move, connect, delete; scroll to zoom, drag to pan | your app (`GraphAction`); pan and zoom in the widget |
| `canvas(w, h, \|f\| ..)` | `Frame` + `Path`: rects, circles, lines, curves, filled polygons, text, clip | n/a |
| `viewport3d([object(mesh)..])` | lit meshes, orbit camera, software-projected through the 2D renderer | camera in the widget |
| `media(source, playback)` | player chrome over any `MediaSource` | your app (`Playback`) |

## Notes and limits

- **Polygon fills** are a new draw primitive (`DrawCommand::Polygon`): ear-clipped into triangles in the renderer. Outline edges are anti-aliased; interior seams are exact (no translucent hairlines). Self-intersecting outlines and holes are not supported, so a donut slice is one simple polygon and a full ring is split in two.
- **The 3D viewport is software**: painter's-algorithm depth sort and flat lighting, so intersecting meshes can show sorting artefacts. It suits previews and gizmos, not scenes. Sharing a live wgpu texture with the window renderer is not implemented; `Frame::image` places a renderer-registered image.
- **Media has no decoder.** Alloy ships the player (seek, volume, keyboard, clock) and the `MediaSource` trait; a decoder crate implements it. The position is a pure function of the clock, so a paused player costs nothing and a playing one only redraws.
- **Idle cost**: `stream` glides new samples in for one interval, then idles; `.stepped()` redraws once per sample. `viewport3d(..).spin(..)` and a playing `media` keep ticking by design; everything else is idle at rest.
