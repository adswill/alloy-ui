#!/usr/bin/env python3
"""Linux, Xvfb, software rendering. First paint = process spawn until the window centre is no longer black
(polled with ImageMagick, ~50 ms resolution), then idle CPU % and RSS over 5 s after a 3 s settle. Usage: measure.py bin..."""
import os, subprocess, sys, time, json

def cpu(pid):
    f = open(f"/proc/{pid}/stat").read().rsplit(")", 1)[1].split()
    return (int(f[11]) + int(f[12])) / os.sysconf("SC_CLK_TCK")

def kids(pid):
    out = subprocess.run(["pgrep", "-P", str(pid)], capture_output=True, text=True).stdout.split()
    return [pid] + [int(x) for x in out]

def tree_cpu(pid):
    return sum(cpu(p) for p in kids(pid) if os.path.exists(f"/proc/{p}"))

def rss(pid):
    t = 0
    for p in kids(pid):
        try:
            t += int([l for l in open(f"/proc/{p}/status") if l.startswith("VmRSS")][0].split()[1])
        except Exception:
            pass
    return t / 1024

def run(binary):
    env = dict(os.environ, WEBKIT_DISABLE_COMPOSITING_MODE="1")
    t0 = time.time()
    p = subprocess.Popen([binary], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    mapped = None
    while time.time() - t0 < 20:
        r = subprocess.run(["import", "-window", "root", "-crop", "4x4+480+320", "txt:-"], capture_output=True, text=True)
        if "#000000" not in r.stdout and "#" in r.stdout:
            mapped = time.time() - t0
            break
    time.sleep(3)
    a, ta = tree_cpu(p.pid), time.time()
    time.sleep(5)
    b, tb = tree_cpu(p.pid), time.time()
    mem = rss(p.pid)
    p.kill()
    return {"bin": os.path.basename(os.path.dirname(os.path.dirname(os.path.dirname(binary)))) + "/" + os.path.basename(binary),
            "size_mb": round(os.path.getsize(binary) / 1e6, 2),
            "window_ms": None if mapped is None else round(mapped * 1000),
            "idle_cpu_pct": round(100 * (b - a) / (tb - ta), 2), "rss_mb": round(mem)}

if __name__ == "__main__":
    for b in sys.argv[1:]:
        print(json.dumps(run(b)), flush=True)
