#!/usr/bin/env python3
"""Open Rivet on a folder of 100,000 empty files and measure, like the Flea benchmark:

  first window  time until the compositor lists Rivet's window (niri or Hyprland),
                falling back to Rivet's own first-frame time if neither is available
  settled       time until the process has used no CPU for 500 ms
  PSS           proportional memory at settle (/proc/PID/smaps_rollup)
  CPU           user + system CPU time used up to settle

Usage: python3 docs/bench-100k.py [--runs 3] [--views list,icons] [--files 100000]
Run from the repository; needs a release build (cargo build --release).
"""
import argparse, json, os, shutil, statistics, subprocess, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
BIN = os.path.join(ROOT, "target", "release", "rivet")
TICK = os.sysconf("SC_CLK_TCK")
QUIET = 0.5


def make_fixture(path, n):
    marker = os.path.join(path, f".rivet-fixture-{n}")
    if os.path.exists(marker):
        return
    os.makedirs(path, exist_ok=True)
    for i in range(n):
        open(os.path.join(path, f"file-{i:06d}"), "w").close()
    open(marker, "w").close()


def compositor_has(pid):
    for cmd in (["niri", "msg", "--json", "windows"], ["hyprctl", "clients", "-j"]):
        if not shutil.which(cmd[0]):
            continue
        try:
            out = subprocess.run(cmd, capture_output=True, text=True, timeout=2).stdout
            return any(w.get("pid") == pid for w in json.loads(out))
        except Exception:
            return None
    return None  # no supported compositor


def cpu_ticks(pid):
    with open(f"/proc/{pid}/stat") as f:
        fields = f.read().rsplit(")", 1)[1].split()
    # utime, stime, cutime, cstime (fields 14-17 counting from 1)
    return sum(int(x) for x in fields[11:15])


def pss_mib(pid):
    with open(f"/proc/{pid}/smaps_rollup") as f:
        for line in f:
            if line.startswith("Pss:"):
                return int(line.split()[1]) / 1024
    return float("nan")


def run_once(folder, view):
    env = dict(os.environ, RIVET_FRAME_STATS="1")
    t0 = time.monotonic()
    p = subprocess.Popen([BIN, folder, "--view", view, "--measure", "30"],
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=env)
    first = None
    use_compositor = True
    last_ticks, last_change = 0, t0
    try:
        while True:
            now = time.monotonic()
            if now - t0 > 25:
                raise RuntimeError("did not settle within 25 s")
            ticks = cpu_ticks(p.pid)
            if ticks != last_ticks:
                last_ticks, last_change = ticks, now
            if first is None and use_compositor:
                seen = compositor_has(p.pid)
                if seen is None:
                    use_compositor = False
                elif seen:
                    first = now - t0
            if first is not None and now - last_change >= QUIET:
                return {
                    "first": first,
                    "settled": last_change - t0,
                    "pss": pss_mib(p.pid),
                    "cpu": last_ticks / TICK,
                }
            time.sleep(0.01)
    finally:
        p.terminate()
        p.wait(timeout=10)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--views", default="list,icons")
    ap.add_argument("--files", type=int, default=100_000)
    ap.add_argument("--folder", default=os.path.join(ROOT, "target", "bench-100k"))
    a = ap.parse_args()
    if not os.access(BIN, os.X_OK):
        sys.exit("Build first: cargo build --release")
    make_fixture(a.folder, a.files)
    version = subprocess.run([BIN, "--version"], capture_output=True, text=True).stdout.strip()
    print(f"{version} — {a.files:,} empty files, {a.runs} runs per view, medians\n")
    print("| View | First window | Settled | PSS | CPU |")
    print("| --- | ---: | ---: | ---: | ---: |")
    for view in a.views.split(","):
        rs = [run_once(a.folder, view) for _ in range(a.runs)]
        med = {k: statistics.median(r[k] for r in rs) for k in rs[0]}
        print(f"| {view} | {med['first']*1000:.0f} ms | {med['settled']:.2f} s | {med['pss']:.1f} MiB | {med['cpu']:.2f} s |")
        print("|  ↳ runs | " + " · ".join(f"{r['first']*1000:.0f}" for r in rs) + " | "
              + " · ".join(f"{r['settled']:.2f}" for r in rs) + " | "
              + " · ".join(f"{r['pss']:.0f}" for r in rs) + " | "
              + " · ".join(f"{r['cpu']:.2f}" for r in rs) + " |", file=sys.stderr)


if __name__ == "__main__":
    main()
