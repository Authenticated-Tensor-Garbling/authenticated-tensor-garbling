#!/usr/bin/env python3
"""Parse benchmark results into the paper's figure layout.

Reads three data sources:

1. A bench log produced by `cargo bench --bench benchmarks 2>&1 | tee bench-….log`,
   which contains one `KB,p{1|2},N=…,M=…,tile=…,kb=…` line per online cell and
   one `KB,prep,N=…,M=…,tile=…,B=…,kb=…` line per preprocessing cell that ran.
2. Criterion's per-cell `target/criterion/online/<bench_id>/<tile>/new/estimates.json`,
   which carries mean / CI for the wall-clock time in nanoseconds.
3. The same for `target/criterion/preprocessing/<bench_id>/<tile>/new/estimates.json`.

Joins them on (protocol, N, M, tile) (or (N, M, tile) for prep), writes a
merged `results.csv`, and emits SIX single-bar PDFs per requested size — one
per (protocol, metric) pair so each chart's y-axis scales to its own data
range and the protocol-vs-protocol comparison happens in print layout (figures
placed side-by-side):

    {N}x{N}_p1_wallclock.pdf       — online P1 timing
    {N}x{N}_p1_communication.pdf   — online P1 KB
    {N}x{N}_p2_wallclock.pdf       — online P2 timing
    {N}x{N}_p2_communication.pdf   — online P2 KB
    {N}x{N}_prep_wallclock.pdf     — preprocessing timing
    {N}x{N}_prep_communication.pdf — preprocessing KB

Bar coloring follows the paper's convention (appendix_experiments.tex §Results):
tile 1 (= distributed half-gates baseline) red, tile 6 (= optimum) blue,
others light gray. The same per-bar coloring applies across all six PDFs
so a single visual key carries through the whole figure set.

Note: an earlier revision plotted a separate red "Standard" (CWYY dual-exec)
reference bar at x=0; that bar has been removed and the red color is now
applied directly to the tile=1 pair, which is the equivalent half-gates
baseline configuration of our protocol.

Usage:
    python3 tools/parse_results.py                               # auto-detect latest bench-*.log
    python3 tools/parse_results.py --log bench-20260426-1901.log
    python3 tools/parse_results.py --sizes 64,128,256            # default — paper's headline sizes
    python3 tools/parse_results.py --sizes all                   # every size that ran
    python3 tools/parse_results.py --no-plots                    # CSV only

Requires: matplotlib (everything else is stdlib).
"""

from __future__ import annotations

import argparse
import csv
import json
import re
import sys
from pathlib import Path
from typing import Optional

KB_LINE = re.compile(
    # `kappa=…,rho=…` are optional so old logs (pre-parameterization) still parse.
    # New benches always emit them — see `gc_bytes_p2` / KB log lines in
    # `benches/benchmarks.rs`. When absent we leave the columns blank.
    r"^KB,p(?P<proto>[12]),N=(?P<n>\d+),M=(?P<m>\d+),tile=(?P<tile>\d+)"
    r"(?:,kappa=(?P<kappa>\d+),rho=(?P<rho>\d+))?"
    r",kb=(?P<kb>[\d.]+)\s*$"
)

# `KB,prep,…` lines from `bench_preprocessing`. Pre-3.6 logs emitted a single
# `B=…` field with no tile sweep (chunking_factor was hardcoded at 1); 3.6+
# logs sweep tile 1..=8 and emit the full mirror of the online schema.
# `B` and `kappa/rho` are accepted in any order via a non-capturing alternation
# group repeated; we just look for each field independently.
KB_LINE_PREP = re.compile(
    r"^KB,prep,N=(?P<n>\d+),M=(?P<m>\d+)"
    r"(?:,tile=(?P<tile>\d+))?"
    r"(?:,kappa=(?P<kappa>\d+),rho=(?P<rho>\d+))?"
    r"(?:,B=(?P<bucket>\d+))?"
    r",kb=(?P<kb>[\d.]+)\s*$"
)

CRITERION_PATH = re.compile(
    r"online_p(?P<proto>[12])/p[12]_garble_eval_check_(?P<n>\d+)x(?P<m>\d+)/(?P<tile>\d+)/new/estimates\.json$"
)

# 3.6+ bench layout: `preprocessing/real_preprocessing_<N>x<M>/<tile>/`. Pre-3.6
# layout lacked the tile axis (`real_preprocessing/<N>x<M>/`); we accept both
# so the parser still works on legacy logs.
CRITERION_PREP_PATH = re.compile(
    r"preprocessing/real_preprocessing(?:_(?P<n_in_name>\d+)x(?P<m_in_name>\d+))?"
    r"/(?P<seg2>[^/]+)"
    r"(?:/(?P<seg3>[^/]+))?"
    r"/new/estimates\.json$"
)

PAPER_SIZES_DEFAULT = [64, 128, 256]
TILES = list(range(1, 9))
OPTIMAL_TILE = 6
BASELINE_TILE = 1
COLOR_BASELINE = "#d62728"  # paper's "red" — tile=1 (half-gates baseline configuration)
COLOR_OPTIMAL = "#1f77b4"   # paper's "blue" — tile=6
COLOR_DEFAULT = "#bdbdbd"   # gray — every other tile


def parse_kb(
    log_path: Path,
) -> tuple[
    dict[tuple[int, int, int, int], float],
    dict[tuple[int, int, int, int], tuple[Optional[int], Optional[int]]],
]:
    """Return ({(proto, n, m, tile): kb}, {(proto, n, m, tile): (kappa, rho)}).

    Re-runs in the same log overwrite. `(kappa, rho)` is `(None, None)` for
    pre-parameterization logs that don't carry those fields.
    """
    kb_out: dict[tuple[int, int, int, int], float] = {}
    params_out: dict[tuple[int, int, int, int], tuple[Optional[int], Optional[int]]] = {}
    with open(log_path) as f:
        for line in f:
            m = KB_LINE.match(line.strip())
            if not m:
                continue
            key = (int(m["proto"]), int(m["n"]), int(m["m"]), int(m["tile"]))
            kb_out[key] = float(m["kb"])
            kappa = int(m["kappa"]) if m["kappa"] is not None else None
            rho   = int(m["rho"])   if m["rho"]   is not None else None
            params_out[key] = (kappa, rho)
    return kb_out, params_out


def parse_ms(criterion_root: Path) -> dict[tuple[int, int, int, int], tuple[float, float, float]]:
    """Return {(proto, n, m, tile): (mean_ms, ci_low_ms, ci_high_ms)}."""
    out: dict[tuple[int, int, int, int], tuple[float, float, float]] = {}
    if not criterion_root.is_dir():
        return out
    for path in criterion_root.rglob("estimates.json"):
        rel = str(path).replace("\\", "/")
        m = CRITERION_PATH.search(rel)
        if not m or "/new/" not in rel:
            continue
        key = (int(m["proto"]), int(m["n"]), int(m["m"]), int(m["tile"]))
        with open(path) as f:
            data = json.load(f)
        mean_block = data["mean"]
        mean_ns = mean_block["point_estimate"]
        ci = mean_block["confidence_interval"]
        out[key] = (mean_ns / 1e6, ci["lower_bound"] / 1e6, ci["upper_bound"] / 1e6)
    return out


def parse_kb_prep(
    log_path: Path,
) -> tuple[
    dict[tuple[int, int, int], float],
    dict[tuple[int, int, int], tuple[Optional[int], Optional[int], Optional[int]]],
]:
    """Return ({(n, m, tile): kb}, {(n, m, tile): (kappa, rho, B)}).

    Pre-3.6 logs that emitted a single `B=…` line with no tile axis are
    bucketed under tile=1 (the chunking factor that was hardcoded then),
    so legacy logs still produce one cell per `(n, m)`.
    """
    kb_out: dict[tuple[int, int, int], float] = {}
    params_out: dict[
        tuple[int, int, int], tuple[Optional[int], Optional[int], Optional[int]]
    ] = {}
    with open(log_path) as f:
        for line in f:
            m = KB_LINE_PREP.match(line.strip())
            if not m:
                continue
            tile = int(m["tile"]) if m["tile"] is not None else 1
            key = (int(m["n"]), int(m["m"]), tile)
            kb_out[key] = float(m["kb"])
            kappa  = int(m["kappa"])  if m["kappa"]  is not None else None
            rho    = int(m["rho"])    if m["rho"]    is not None else None
            bucket = int(m["bucket"]) if m["bucket"] is not None else None
            params_out[key] = (kappa, rho, bucket)
    return kb_out, params_out


def parse_ms_prep(
    criterion_prep_root: Path,
) -> dict[tuple[int, int, int], tuple[float, float, float]]:
    """Return {(n, m, tile): (mean_ms, ci_low_ms, ci_high_ms)}.

    Walks `target/criterion/preprocessing/` looking for `estimates.json`
    under both the 3.6+ tile-aware layout
    (`real_preprocessing_<N>x<M>/<tile>/new/`) and the pre-3.6 layout
    (`real_preprocessing/<N>x<M>/new/`, bucketed under tile=1).
    """
    out: dict[tuple[int, int, int], tuple[float, float, float]] = {}
    if not criterion_prep_root.is_dir():
        return out
    for path in criterion_prep_root.rglob("estimates.json"):
        rel = str(path).replace("\\", "/")
        if "/new/" not in rel:
            continue
        m = CRITERION_PREP_PATH.search(rel)
        if not m:
            continue
        # 3.6+ layout: name carries N×M, seg2 = tile.
        # Pre-3.6 layout: name is bare "real_preprocessing", seg2 = "<N>x<M>".
        if m["n_in_name"] is not None and m["m_in_name"] is not None:
            n, mm = int(m["n_in_name"]), int(m["m_in_name"])
            try:
                tile = int(m["seg2"])
            except (TypeError, ValueError):
                continue
        else:
            seg2 = m["seg2"] or ""
            if "x" not in seg2:
                continue
            try:
                n_str, m_str = seg2.split("x", 1)
                n, mm = int(n_str), int(m_str)
            except ValueError:
                continue
            tile = 1  # pre-3.6 only ran chunking_factor=1
        key = (n, mm, tile)
        with open(path) as f:
            data = json.load(f)
        mean_block = data["mean"]
        mean_ns = mean_block["point_estimate"]
        ci = mean_block["confidence_interval"]
        out[key] = (mean_ns / 1e6, ci["lower_bound"] / 1e6, ci["upper_bound"] / 1e6)
    return out


def write_csv(
    out_dir: Path,
    ms_data, kb_data, params_data,
    ms_prep=None, kb_prep=None, params_prep=None,
) -> None:
    """Emit `results.csv` with one row per cell.

    Online cells: protocol = "p1" / "p2", populated kappa/rho.
    Preprocessing cells: protocol = "prep", populated B (the bucket size,
    re-using the `kappa` column slot ... no — added as a separate column).
    """
    ms_prep = ms_prep or {}
    kb_prep = kb_prep or {}
    params_prep = params_prep or {}

    online_keys = sorted(set(ms_data.keys()) | set(kb_data.keys()))
    prep_keys = sorted(set(ms_prep.keys()) | set(kb_prep.keys()))
    path = out_dir / "results.csv"
    with open(path, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow([
            "protocol", "N", "M", "tile",
            "ms_mean", "ms_ci_low", "ms_ci_high", "kb",
            "kappa", "rho", "B",
        ])
        for k in online_keys:
            mean, low, high = ms_data.get(k, (None, None, None))
            kb = kb_data.get(k)
            kappa, rho = params_data.get(k, (None, None))
            w.writerow([
                f"p{k[0]}", k[1], k[2], k[3],
                mean, low, high, kb,
                kappa, rho, "",
            ])
        for k in prep_keys:
            n, mm, tile = k
            mean, low, high = ms_prep.get(k, (None, None, None))
            kb = kb_prep.get(k)
            kappa, rho, bucket = params_prep.get(k, (None, None, None))
            w.writerow([
                "prep", n, mm, tile,
                mean, low, high, kb,
                kappa, rho, bucket,
            ])
    print(f"  wrote {path} ({len(online_keys)} online + {len(prep_keys)} prep rows)")


def bar_color(tile: int) -> str:
    """Color for an our-protocol tile bar. tile=1 is the half-gates
    baseline configuration (red); tile=6 is the optimum (blue); every
    other tile is gray."""
    if tile == OPTIMAL_TILE:
        return COLOR_OPTIMAL
    if tile == BASELINE_TILE:
        return COLOR_BASELINE
    return COLOR_DEFAULT


BAR_WIDTH = 0.4


def plot_single_bar(
    out_path: Path,
    pairs: list[tuple[int, float]],
    ylabel: str,
) -> None:
    """Single-bar-per-tile chart for the preprocessing-only figures.

    Same paper-style tile-color encoding (red=tile-1 half-gates baseline,
    blue=tile-6 paper optimum, gray otherwise) so the prep chart's color
    semantics stay consistent with the online charts. No hatch — there's
    no in-cluster competitor to distinguish from, so the visual marker
    used in the 3-bar layout is unnecessary noise here.
    """
    import matplotlib.pyplot as plt

    pairs_dict = dict(pairs)
    all_tiles = sorted(pairs_dict)
    if not all_tiles:
        return

    fig, ax = plt.subplots(figsize=(5.0, 3.0))

    x = list(all_tiles)
    v = [pairs_dict[t] for t in x]
    c = [bar_color(t) for t in x]

    ax.bar(x, v, width=BAR_WIDTH, color=c,
           edgecolor="black", linewidth=0.6)

    ax.set_xlabel("Tile size")
    ax.set_ylabel(ylabel)
    ax.set_xticks(all_tiles)
    ax.margins(x=0.05)
    ax.spines["top"].set_visible(False)
    ax.spines["right"].set_visible(False)

    fig.tight_layout()
    fig.savefig(out_path, bbox_inches="tight")
    plt.close(fig)


def plot_size(
    ms_data, kb_data,
    n: int, out_dir: Path,
    ms_prep=None, kb_prep=None,
) -> tuple[int, int]:
    """Six single-bar PDFs per size n×n — one per (protocol, metric):

        - {n}x{n}_p1_wallclock.pdf       — online P1 timing
        - {n}x{n}_p1_communication.pdf   — online P1 KB
        - {n}x{n}_p2_wallclock.pdf       — online P2 timing
        - {n}x{n}_p2_communication.pdf   — online P2 KB
        - {n}x{n}_prep_wallclock.pdf     — preprocessing timing
        - {n}x{n}_prep_communication.pdf — preprocessing KB

    Each PDF is a single-bar-per-tile chart so the protocol-vs-protocol
    comparison happens in print layout (figures placed side-by-side) rather
    than packed into a single chart. Per-bar tile coloring stays consistent
    across all six (red=tile-1 half-gates baseline, blue=tile-6 paper
    optimum, gray otherwise) so the per-bar reading is the same regardless
    of which figure you're looking at.

    Returns `(online_pdfs_written, prep_pdfs_written)`.
    """
    ms_prep = ms_prep or {}
    kb_prep = kb_prep or {}

    ms_p1 = [(t, ms_data[(1, n, n, t)][0]) for t in TILES if (1, n, n, t) in ms_data]
    ms_p2 = [(t, ms_data[(2, n, n, t)][0]) for t in TILES if (2, n, n, t) in ms_data]
    kb_p1 = [(t, kb_data[(1, n, n, t)])    for t in TILES if (1, n, n, t) in kb_data]
    kb_p2 = [(t, kb_data[(2, n, n, t)])    for t in TILES if (2, n, n, t) in kb_data]

    ms_pp = [(t, ms_prep[(n, n, t)][0]) for t in TILES if (n, n, t) in ms_prep]
    kb_pp = [(t, kb_prep[(n, n, t)])    for t in TILES if (n, n, t) in kb_prep]

    online_written = 0
    if ms_p1:
        plot_single_bar(out_dir / f"{n}x{n}_p1_wallclock.pdf",     ms_p1, "Time (ms)")
        online_written += 1
    if kb_p1:
        plot_single_bar(out_dir / f"{n}x{n}_p1_communication.pdf", kb_p1, "Comm (KB)")
        online_written += 1
    if ms_p2:
        plot_single_bar(out_dir / f"{n}x{n}_p2_wallclock.pdf",     ms_p2, "Time (ms)")
        online_written += 1
    if kb_p2:
        plot_single_bar(out_dir / f"{n}x{n}_p2_communication.pdf", kb_p2, "Comm (KB)")
        online_written += 1

    prep_written = 0
    if ms_pp:
        plot_single_bar(out_dir / f"{n}x{n}_prep_wallclock.pdf",     ms_pp, "Time (ms)")
        prep_written += 1
    if kb_pp:
        plot_single_bar(out_dir / f"{n}x{n}_prep_communication.pdf", kb_pp, "Comm (KB)")
        prep_written += 1

    return online_written, prep_written


def auto_detect_log() -> Optional[Path]:
    candidates = sorted(Path(".").glob("bench-*.log"))
    return candidates[-1] if candidates else None


def parse_sizes_arg(arg: str, all_sizes_in_data: list[int]) -> list[int]:
    if arg.lower() == "all":
        return all_sizes_in_data
    return [int(s) for s in arg.split(",") if s.strip()]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--log", type=Path, default=None, help="bench log file (default: latest bench-*.log)")
    ap.add_argument("--criterion-root", type=Path, default=Path("target/criterion/online"))
    ap.add_argument("--criterion-prep-root", type=Path,
                    default=Path("target/criterion/preprocessing"),
                    help="criterion output root for the preprocessing group")
    ap.add_argument("--out", type=Path, default=Path("figures"))
    ap.add_argument("--sizes", default=",".join(str(s) for s in PAPER_SIZES_DEFAULT),
                    help="comma-separated N values (square sizes), or 'all' (default: paper's 64,128,256)")
    ap.add_argument("--no-plots", action="store_true", help="write CSV only, skip PDFs")
    args = ap.parse_args()

    log_path = args.log or auto_detect_log()
    if log_path is None or not log_path.exists():
        print("error: no bench log found. pass --log <path>.", file=sys.stderr)
        return 2
    if not args.criterion_root.is_dir():
        print(f"error: {args.criterion_root} not found. run the bench first.", file=sys.stderr)
        return 2

    args.out.mkdir(parents=True, exist_ok=True)

    print(f"reading KB lines from {log_path}")
    kb_data, params_data = parse_kb(log_path)
    kb_prep, params_prep = parse_kb_prep(log_path)
    print(f"reading Criterion ms estimates under {args.criterion_root}")
    ms_data = parse_ms(args.criterion_root)
    if args.criterion_prep_root.is_dir():
        print(f"reading Criterion prep ms estimates under {args.criterion_prep_root}")
        ms_prep = parse_ms_prep(args.criterion_prep_root)
    else:
        print(f"  note: {args.criterion_prep_root} not found; prep ms columns will be blank")
        ms_prep = {}

    n_kb, n_ms = len(kb_data), len(ms_data)
    n_joined = len(set(kb_data) & set(ms_data))
    print(f"  online — KB cells: {n_kb}   ms cells: {n_ms}   joined: {n_joined}")
    n_prep_kb, n_prep_ms = len(kb_prep), len(ms_prep)
    n_prep_joined = len(set(kb_prep) & set(ms_prep))
    print(f"  prep   — KB cells: {n_prep_kb}   ms cells: {n_prep_ms}   joined: {n_prep_joined}")
    distinct_params = sorted({
        p for p in params_data.values() if p != (None, None)
    })
    if distinct_params:
        print(f"  (κ, ρ) seen: {distinct_params}")

    write_csv(args.out, ms_data, kb_data, params_data,
              ms_prep=ms_prep, kb_prep=kb_prep, params_prep=params_prep)

    if args.no_plots:
        return 0

    sizes_in_data = sorted({
        n for (_p, n, m, _t) in (set(kb_data) | set(ms_data)) if n == m
    } | {
        n for (n, m, _t) in (set(kb_prep) | set(ms_prep)) if n == m
    })
    sizes = parse_sizes_arg(args.sizes, sizes_in_data)

    n_online, n_prep = 0, 0
    for n in sizes:
        online_w, prep_w = plot_size(ms_data, kb_data, n, args.out,
                                     ms_prep=ms_prep, kb_prep=kb_prep)
        n_online += online_w
        n_prep   += prep_w
    print(f"  wrote {n_online} online + {n_prep} prep PDFs into {args.out}/")
    return 0


if __name__ == "__main__":
    sys.exit(main())
