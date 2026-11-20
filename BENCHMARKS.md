# Benchmarks

Produced by `cargo run --release -p alloy-ui-bench`. Only numbers that were actually measured are listed.

## What is and is not measured

- **Frame build**: `update` + `view` + layout + draw-list construction through the real `Runtime`.
- **CPU raster**: the software reference renderer at 2x scale. This is *not* the GPU path.
- **Per no-op event**: runtime cost of an input event that changes nothing; a bound on idle work, not process CPU %.
- **Startup**: wall time from spawning the example binary to its first frame written as PNG. The examples run headless because there is no windowed shell yet, so this is not window-visible time.
- **Binary size**: the headless `counter` binary (it links the renderer). The windowed counter will differ.
- **Not measured yet**: real fps, idle CPU %, windowed startup, ProMotion 120 fps. These need the platform shell.

## Results

Run on the cloud development container (shared CPU, so treat as indicative). macOS and Windows: not run yet; run the harness there and add a section.

   Compiling alloy-ui-examples v0.0.1 (/home/claude/alloy-ui/examples)
    Finished `release` profile [optimized] target(s) in 31.12s
### linux / x86_64

| Example | Frame build (median) | CPU raster 2x (median) | Per no-op event |
|---|---|---|---|
| counter | 3 us | 16.8 ms | 1238 ns |
| todo | 3 us | 27.2 ms | 2374 ns |
| text_editor | 2 us | 32.8 ms | 1212 ns |
| settings | 2 us | 41.1 ms | 1325 ns |
| dashboard | 12 us | 41.2 ms | 8042 ns |

| Metric | Value | Target |
|---|---|---|
| counter release binary (headless build) | 1.99 MB | < 10 MB |
| counter spawn to first frame written (headless, median of 7) | 19 ms | < 200 ms |
| fps, idle CPU %, windowed startup | not measured (no platform shell yet) | 60 fps, < 1%, < 200 ms |
