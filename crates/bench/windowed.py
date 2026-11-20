#!/usr/bin/env python3
"""Windowed benchmark for Linux under Xvfb: first paint, idle frames and CPU, frames under input.

usage: windowed.py <release-dir> <example>...      (needs Xvfb, xdotool, python-xlib)

Each example runs twice with ALLOY_PERF=1 (the shell prints `first_paint_ms` and, on a clean
exit, `frames`/`elapsed_ms`):
  * idle: window untouched for 5 s. Reports idle CPU, frames painted and resident memory.
  * moving pointer: xdotool sweeps the pointer across the window for 5 s. Reports frames/s.
A clean exit is a WM_DELETE_WINDOW message, so the shell prints its totals.
Rendering is software (Mesa llvmpipe), so these are NOT GPU numbers.
"""
import os, re, subprocess, sys, time

from Xlib import X, display, protocol

DISP = ":77"
HZ = os.sysconf("SC_CLK_TCK")


def cpu_ticks(pid):
    with open(f"/proc/{pid}/stat") as f:
        p = f.read().rsplit(")", 1)[1].split()
    return int(p[11]) + int(p[12])


def rss_mb(pid):
    with open(f"/proc/{pid}/status") as f:
        for line in f:
            if line.startswith("VmRSS"):
                return int(line.split()[1]) / 1024
    return float("nan")


def window_of(pid, timeout=15):
    end = time.time() + timeout
    while time.time() < end:
        out = subprocess.run(["xdotool", "search", "--pid", str(pid)], capture_output=True, text=True).stdout.split()
        if out:
            return int(out[-1])
        time.sleep(0.02)
    raise SystemExit(f"no window for pid {pid}")


def close_window(wid):
    d = display.Display(DISP)
    w = d.create_resource_object("window", wid)
    msg = protocol.event.ClientMessage(
        window=w,
        client_type=d.intern_atom("WM_PROTOCOLS"),
        data=(32, [d.intern_atom("WM_DELETE_WINDOW"), X.CurrentTime, 0, 0, 0]),
    )
    w.send_event(msg, event_mask=X.NoEventMask)
    d.flush()


def run(binary, move, size):
    env = dict(os.environ, DISPLAY=DISP, ALLOY_PERF="1")
    t0 = time.time()
    p = subprocess.Popen([binary], env=env, stderr=subprocess.PIPE, text=True)
    wid = window_of(p.pid)
    mapped = (time.time() - t0) * 1000
    time.sleep(1.5)
    c0, t1 = cpu_ticks(p.pid), time.time()
    if move:
        x0, y0 = 40, 40
        end = time.time() + 5
        i = 0
        while time.time() < end:
            i += 1
            subprocess.run(["xdotool", "mousemove", "--window", str(wid), str(x0 + (i * 17) % (size[0] - 80)), str(y0 + (i * 11) % (size[1] - 80))])
    else:
        time.sleep(5)
    cpu = (cpu_ticks(p.pid) - c0) * 100 / (HZ * (time.time() - t1))
    rss = rss_mb(p.pid)
    close_window(wid)
    try:
        _, err = p.communicate(timeout=10)
    except subprocess.TimeoutExpired:
        p.kill()
        _, err = p.communicate()
    fp = re.search(r"first_paint_ms=([\d.]+)", err)
    fr = re.search(r"frames=(\d+) elapsed_ms=([\d.]+)", err)
    return dict(
        mapped=mapped,
        first_paint=float(fp.group(1)) if fp else None,
        frames=int(fr.group(1)) if fr else None,
        elapsed=float(fr.group(2)) / 1000 if fr else None,
        cpu=cpu,
        rss=rss,
    )


def main():
    rel, names = sys.argv[1], sys.argv[2:]
    xvfb = subprocess.Popen(["Xvfb", DISP, "-screen", "0", "1280x800x24", "-nolisten", "tcp"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    os.environ["DISPLAY"] = DISP
    sizes = {"counter": (360, 200), "todo": (440, 470), "text_editor": (640, 420), "settings": (640, 420), "dashboard": (640, 560)}
    print("| Example | First paint | Idle frames (≈6.5 s run) | Idle CPU (5 s) | Frames/s, pointer moving | Resident memory |")
    print("|---|---|---|---|---|---|")
    try:
        for n in names:
            b = os.path.join(rel, n)
            idle = run(b, False, sizes[n])
            busy = run(b, True, sizes[n])
            fps = f"{busy['frames'] / busy['elapsed']:.0f}" if busy["frames"] else "n/a"
            frames = idle["frames"] if idle["frames"] is not None else "n/a"
            fp = f"{idle['first_paint']:.0f} ms" if idle["first_paint"] else "n/a"
            print(f"| {n} | {fp} | {frames} | {idle['cpu']:.2f}% | {fps} | {idle['rss']:.0f} MB |", flush=True)
    finally:
        xvfb.terminate()


main()
