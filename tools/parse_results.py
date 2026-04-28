#!/usr/bin/env python3
"""Parse benchmark results into the paper's figure layout.

Reads two data sources:

1. A bench log produced by `cargo bench --bench benchmarks 2>&1 | tee bench-….log`,
   which contains one `KB,p{1|2},N=…,M=…,tile=…,kb=…` line per cell that ran.
2. Criterion's per-cell `target/criterion/online/<bench_id>/<tile>/new/estimates.json`,
   which carries mean / CI for the wall-clock time in nanoseconds.

Joins the two on (protocol, N, M, tile), writes a merged `results.csv`, and emits
one PDF per (size, metric) with both protocols' bars side-by-side:

    {N}x{N}_wallclock_bar.pdf     – ms per tile size, P2 on left, P1 on right
    {N}x{N}_communication.pdf    – KB per tile size, same layout

Bar coloring follows the paper's convention (appendix_experiments.tex §Results):
tile 1 (= distributed half-gates baseline) red, tile 6 (= optimum) blue,
others light gray. Within each tile pair, P2 (left) uses the full tile
color and P1 (right) uses a lightened version of the same color — same hue
family, different saturation. The protocol legend is emitted only on the
64×64 wallclock panel (LEGEND_CELL); the other panels rely on the same
left=darker=P2, right=lighter=P1 convention.

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

CRITERION_PATH = re.compile(
    r"online/p(?P<proto>[12])_garble_eval_check_(?P<n>\d+)x(?P<m>\d+)/(?P<tile>\d+)/new/estimates\.json$"
)

PAPER_SIZES_DEFAULT = [64, 128, 256]
TILES = list(range(1, 9))
OPTIMAL_TILE = 6
BASELINE_TILE = 1
COLOR_BASELINE = "#d62728"  # paper's "red" — tile=1 (half-gates baseline configuration)
COLOR_OPTIMAL = "#1f77b4"   # paper's "blue" — tile=6
COLOR_DEFAULT = "#bdbdbd"   # gray — every other tile
LIGHTEN_FRAC = 0.55         # P1 = blend toward white by this fraction


def lighten(hex_color: str, frac: float = LIGHTEN_FRAC) -> str:
    """Blend a #rrggbb color toward white by `frac` (0 = unchanged, 1 = white)."""
    r, g, b = (int(hex_color[i:i+2], 16) for i in (1, 3, 5))
    r = int(r + (255 - r) * frac)
    g = int(g + (255 - g) * frac)
    b = int(b + (255 - b) * frac)
    return f"#{r:02x}{g:02x}{b:02x}"


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


def write_csv(out_dir: Path, ms_data, kb_data, params_data) -> None:
    keys = sorted(set(ms_data.keys()) | set(kb_data.keys()))
    path = out_dir / "results.csv"
    with open(path, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow([
            "protocol", "N", "M", "tile",
            "ms_mean", "ms_ci_low", "ms_ci_high", "kb",
            "kappa", "rho",
        ])
        for k in keys:
            mean, low, high = ms_data.get(k, (None, None, None))
            kb = kb_data.get(k)
            kappa, rho = params_data.get(k, (None, None))
            w.writerow([
                f"p{k[0]}", k[1], k[2], k[3],
                mean, low, high, kb,
                kappa, rho,
            ])
    print(f"  wrote {path} ({len(keys)} rows)")


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
BAR_GAP = 0.02  # small gap between P2 and P1 within a tile group


def plot_grouped_bar(
    out_path: Path,
    p2_pairs: list[tuple[int, float]],
    p1_pairs: list[tuple[int, float]],
    ylabel: str,
    show_legend: bool = False,
) -> None:
    """Grouped bar chart with one (P2 left, P1 right) pair per tile.

    P2 uses the tile-color; P1 uses a lightened version (blended toward
    white by ``LIGHTEN_FRAC``) so the protocol pair stays in the same hue
    family while remaining distinguishable.

    Tile coloring: tile=1 = red (half-gates baseline configuration);
    tile=6 = blue (optimum); every other tile = gray.
    """
    import matplotlib.pyplot as plt
    from matplotlib.patches import Patch

    p2_dict = dict(p2_pairs)
    p1_dict = dict(p1_pairs)
    all_tiles = sorted(set(p2_dict) | set(p1_dict))
    if not all_tiles:
        return

    fig, ax = plt.subplots(figsize=(5.0, 3.0))

    half_offset = BAR_WIDTH / 2 + BAR_GAP / 2
    p2_x = [t - half_offset for t in all_tiles if t in p2_dict]
    p2_v = [p2_dict[t] for t in all_tiles if t in p2_dict]
    p2_c = [bar_color(t) for t in all_tiles if t in p2_dict]

    p1_x = [t + half_offset for t in all_tiles if t in p1_dict]
    p1_v = [p1_dict[t] for t in all_tiles if t in p1_dict]
    p1_c = [lighten(bar_color(t)) for t in all_tiles if t in p1_dict]

    ax.bar(p2_x, p2_v, width=BAR_WIDTH, color=p2_c,
           edgecolor="black", linewidth=0.6)
    ax.bar(p1_x, p1_v, width=BAR_WIDTH, color=p1_c,
           edgecolor="black", linewidth=0.6)

    ax.set_xlabel("Tile size")
    ax.set_ylabel(ylabel)
    ax.set_xticks(all_tiles)
    ax.margins(x=0.05)
    ax.spines["top"].set_visible(False)
    ax.spines["right"].set_visible(False)

    if show_legend:
        # Gray tile color at full vs lightened communicates the P2/P1 split
        # that holds across every tile pair regardless of color.
        legend_handles = [
            Patch(facecolor=COLOR_DEFAULT, edgecolor="black", label="P2 (left)"),
            Patch(facecolor=lighten(COLOR_DEFAULT), edgecolor="black", label="P1 (right)"),
        ]
        ax.legend(handles=legend_handles, loc="upper right", fontsize=8, framealpha=0.9)

    fig.tight_layout()
    fig.savefig(out_path, bbox_inches="tight")
    plt.close(fig)


# The single chart that carries the protocol legend — paper-style "explain it
# once". Picked as the smallest paper size's wallclock panel.
LEGEND_CELL = (64, "wallclock")


def plot_size(ms_data, kb_data, n: int, out_dir: Path) -> tuple[bool, bool]:
    """One combined chart per metric for size n×n. Returns (wc_written, comm_written)."""
    ms_p2 = [(t, ms_data[(2, n, n, t)][0]) for t in TILES if (2, n, n, t) in ms_data]
    ms_p1 = [(t, ms_data[(1, n, n, t)][0]) for t in TILES if (1, n, n, t) in ms_data]
    kb_p2 = [(t, kb_data[(2, n, n, t)]) for t in TILES if (2, n, n, t) in kb_data]
    kb_p1 = [(t, kb_data[(1, n, n, t)]) for t in TILES if (1, n, n, t) in kb_data]

    wallclock_written = False
    if ms_p2 or ms_p1:
        plot_grouped_bar(
            out_dir / f"{n}x{n}_wallclock_bar.pdf",
            ms_p2, ms_p1, "Time (ms)",
            show_legend=(n, "wallclock") == LEGEND_CELL,
        )
        wallclock_written = True

    comm_written = False
    if kb_p2 or kb_p1:
        plot_grouped_bar(
            out_dir / f"{n}x{n}_communication.pdf",
            kb_p2, kb_p1, "Comm (KB)",
            show_legend=(n, "communication") == LEGEND_CELL,
        )
        comm_written = True

    return wallclock_written, comm_written


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
    print(f"reading Criterion ms estimates under {args.criterion_root}")
    ms_data = parse_ms(args.criterion_root)

    n_kb, n_ms = len(kb_data), len(ms_data)
    n_joined = len(set(kb_data) & set(ms_data))
    print(f"  KB cells: {n_kb}   ms cells: {n_ms}   joined: {n_joined}")
    distinct_params = sorted({
        p for p in params_data.values() if p != (None, None)
    })
    if distinct_params:
        print(f"  (κ, ρ) seen: {distinct_params}")

    write_csv(args.out, ms_data, kb_data, params_data)

    if args.no_plots:
        return 0

    sizes_in_data = sorted({n for (_p, n, m, _t) in (set(kb_data) | set(ms_data)) if n == m})
    sizes = parse_sizes_arg(args.sizes, sizes_in_data)

    n_wc, n_comm = 0, 0
    for n in sizes:
        wc, comm = plot_size(ms_data, kb_data, n, args.out)
        n_wc += int(wc)
        n_comm += int(comm)
    print(f"  wrote {n_wc} wallclock + {n_comm} communication PDFs into {args.out}/")
    return 0


if __name__ == "__main__":
    sys.exit(main())
