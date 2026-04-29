#!/usr/bin/env python3
"""Generate the per-protocol Standard-vs-Performance comparison table.

Reads the same two data sources `parse_results.py` does:

  1. A bench log produced by `cargo bench --bench benchmarks 2>&1 | tee bench-….log`
     (one `KB,p{1|2},N=…,M=…,tile=…[,kappa=…,rho=…],kb=…` line per cell).
  2. Criterion's per-cell `target/criterion/online/<bench_id>/<tile>/new/estimates.json`
     (carries mean / CI for wallclock in nanoseconds).

For each requested square size N×N, each protocol gets three columns
comparing its own tile=1 baseline against its chosen performance tile:

  - **Tile 1**: the protocol at tile=1 — at tile=1 our construction
    degenerates to per-AND-gate authenticated half-gates, so this column
    serves as the protocol's own Standard baseline.
  - **Tile k**: the protocol at the chosen tile (default tile=6, the
    time/communication sweet-spot per the paper's analysis).
  - **Imp.**: improvement ratio Tile 1 / Tile k (per protocol).

Both communication (KB) and wallclock time (ms) are reported per size,
and the comparison is self-contained within each protocol — no external
CWYY dual-execution multiplier.

Output formats:

  - `separate`: one small table per size — drops directly into the paper's
    existing single-cell table style.
  - `combined` (recommended for the paper's combined figure block): one
    stacked table covering all three sizes.
  - `csv`: raw numbers for spreadsheet / sanity-check use.

Usage:
    python3 tools/comparison_table.py                       # combined, tile=6
    python3 tools/comparison_table.py --tile-select comm    # argmin-by-comm instead of fixed-6
    python3 tools/comparison_table.py --format separate
    python3 tools/comparison_table.py --format csv
    python3 tools/comparison_table.py --sizes 64,128,256 --log bench-….log

A single bench run feeds both this tool and `parse_results.py`.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Optional

# Reuse parsing from the figure generator. Single source for the data layer.
sys.path.insert(0, str(Path(__file__).parent))
from parse_results import (  # noqa: E402
    parse_kb,
    parse_ms,
    parse_kb_prep,
    parse_ms_prep,
    auto_detect_log,
)


SIZES_DEFAULT = [64, 128, 256]
TILES = list(range(1, 9))


def select_tile(
    proto: int, n: int, m: int,
    kb_data, ms_data,
    mode: str,
) -> Optional[int]:
    """Pick the tile reported under the per-protocol Performance column.

    mode:
      - "comm": argmin over KB
      - "time": argmin over ms (mean)
      - "<int>": that specific tile, if present
    """
    if mode.isdigit():
        t = int(mode)
        return t if (proto, n, m, t) in kb_data else None

    candidates = [
        t for t in TILES
        if (proto, n, m, t) in kb_data and (proto, n, m, t) in ms_data
    ]
    if not candidates:
        return None

    if mode == "comm":
        return min(candidates, key=lambda t: kb_data[(proto, n, m, t)])
    if mode == "time":
        return min(candidates, key=lambda t: ms_data[(proto, n, m, t)][0])
    raise ValueError(f"unknown tile-select mode: {mode}")


BASELINE_TILE = 1


def select_prep_tile(
    n: int, m: int,
    kb_prep, ms_prep,
    mode: str,
) -> Optional[int]:
    """Tile selection for the preprocessing block.

    Mirrors `select_tile` but operates on prep's `(n, m, tile)` keys (no
    proto axis). For an integer mode, requires the cell exists in `kb_prep`.
    """
    if mode.isdigit():
        t = int(mode)
        return t if (n, m, t) in kb_prep else None
    candidates = [
        t for t in TILES
        if (n, m, t) in kb_prep and (n, m, t) in ms_prep
    ]
    if not candidates:
        return None
    if mode == "comm":
        return min(candidates, key=lambda t: kb_prep[(n, m, t)])
    if mode == "time":
        return min(candidates, key=lambda t: ms_prep[(n, m, t)][0])
    raise ValueError(f"unknown tile-select mode: {mode}")


def gather_row(
    n: int,
    kb_data, ms_data,
    tile_select: str,
    kb_prep=None, ms_prep=None,
) -> Optional[dict]:
    """Compute one row: P1.{Std, Perf, Imp} | P2.{Std, Perf, Imp} | Prep.{Std, Perf, Imp}.

    Each protocol's Standard column = own tile=1 cell; Performance =
    own selected tile; Imp. = Standard / Performance.

    The Prep block reuses the same `tile_select` for parity. Prep KB is
    paper-formula chunking-invariant (`appendix_krrw_pre.tex:495-499`), so
    its Std/Perf KB cells are equal and `prep_imp_kb = 1.00×` — communicating
    the invariance directly. Prep ms IS chunking-dependent, so Std/Perf ms
    differ and `prep_imp_ms` is the meaningful number.

    Prep columns are emitted as `None` when prep data is missing for `n`,
    rather than skipping the row, so a partial bench run still yields the
    P1/P2 comparison.
    """
    kb_prep = kb_prep or {}
    ms_prep = ms_prep or {}

    m = n
    if (1, n, m, BASELINE_TILE) not in kb_data or (2, n, m, BASELINE_TILE) not in kb_data:
        return None

    p1_tile = select_tile(1, n, m, kb_data, ms_data, tile_select)
    p2_tile = select_tile(2, n, m, kb_data, ms_data, tile_select)
    if p1_tile is None or p2_tile is None:
        return None

    p1_std_kb = kb_data[(1, n, m, BASELINE_TILE)]
    p1_std_ms = ms_data.get((1, n, m, BASELINE_TILE), (None,))[0]
    p2_std_kb = kb_data[(2, n, m, BASELINE_TILE)]
    p2_std_ms = ms_data.get((2, n, m, BASELINE_TILE), (None,))[0]

    p1_kb = kb_data[(1, n, m, p1_tile)]
    p1_ms = ms_data.get((1, n, m, p1_tile), (None,))[0]
    p2_kb = kb_data[(2, n, m, p2_tile)]
    p2_ms = ms_data.get((2, n, m, p2_tile), (None,))[0]

    # Prep: optional. Pull Std at tile=1 and Perf at the same tile_select.
    prep_tile = select_prep_tile(n, m, kb_prep, ms_prep, tile_select)
    prep_std_kb = kb_prep.get((n, m, BASELINE_TILE))
    prep_std_ms = ms_prep.get((n, m, BASELINE_TILE), (None,))[0] if (n, m, BASELINE_TILE) in ms_prep else None
    prep_kb = kb_prep.get((n, m, prep_tile)) if prep_tile is not None else None
    prep_ms = ms_prep.get((n, m, prep_tile), (None,))[0] if prep_tile is not None and (n, m, prep_tile) in ms_prep else None

    def imp(num: Optional[float], denom: Optional[float]) -> Optional[float]:
        if num is None or denom is None or denom <= 0:
            return None
        return num / denom

    return {
        "n": n,
        "baseline_tile": BASELINE_TILE,
        "p1_tile": p1_tile,
        "p2_tile": p2_tile,
        "prep_tile": prep_tile,
        "p1_std_kb": p1_std_kb,
        "p1_std_ms": p1_std_ms,
        "p2_std_kb": p2_std_kb,
        "p2_std_ms": p2_std_ms,
        "prep_std_kb": prep_std_kb,
        "prep_std_ms": prep_std_ms,
        "p1_kb": p1_kb,
        "p1_ms": p1_ms,
        "p2_kb": p2_kb,
        "p2_ms": p2_ms,
        "prep_kb": prep_kb,
        "prep_ms": prep_ms,
        "p1_imp_kb": imp(p1_std_kb, p1_kb),
        "p1_imp_ms": imp(p1_std_ms, p1_ms),
        "p2_imp_kb": imp(p2_std_kb, p2_kb),
        "p2_imp_ms": imp(p2_std_ms, p2_ms),
        "prep_imp_kb": imp(prep_std_kb, prep_kb),
        "prep_imp_ms": imp(prep_std_ms, prep_ms),
    }


def fmt(x: Optional[float], digits: int) -> str:
    return f"{x:.{digits}f}" if x is not None else "?"


def header_comment(rows, tile_mode: str) -> str:
    note = (
        "Each protocol's Standard column = own tile=1; "
        "Imp. column = Standard / Performance (within-protocol). "
        "Prep KB is chunking-invariant per paper formula "
        "(appendix_krrw_pre.tex:495-499), so prep_imp_kb=1.00x is expected; "
        "prep_imp_ms is the meaningful prep speedup."
    )
    tile_summary = "tile selected by '{}'".format(tile_mode)
    if rows:
        parts = []
        for r in rows:
            seg = "N={}: P1@tile={}, P2@tile={}".format(
                r["n"], r["p1_tile"], r["p2_tile"]
            )
            if r.get("prep_tile") is not None:
                seg += ", Prep@tile={}".format(r["prep_tile"])
            parts.append(seg)
        tile_summary = f"{tile_summary} ({', '.join(parts)})"
    return (
        "% Generated by tools/comparison_table.py — {note}; {ts}."
    ).format(note=note, ts=tile_summary)


def _perf_label(rows, key: str) -> str:
    """Header label for a protocol's perf column.

    'Tile k' if every row picked the same k for this protocol; otherwise
    'Best tile' (the per-row tiles are listed in the comment header)."""
    tiles = {r[key] for r in rows if r.get(key) is not None}
    if not tiles:
        return "Tile ?"
    if len(tiles) == 1:
        return f"Tile {next(iter(tiles))}"
    return "Best tile"


def _has_any_prep(rows) -> bool:
    """True if at least one row has prep KB or ms data."""
    return any(
        r.get("prep_std_kb") is not None
        or r.get("prep_kb") is not None
        or r.get("prep_std_ms") is not None
        or r.get("prep_ms") is not None
        for r in rows
    )


def emit_separate(rows, tile_mode: str) -> None:
    """One table per size.

    Without prep:  7-column layout — [label, P1.{Std,Perf,Imp}, P2.{Std,Perf,Imp}]
    With prep:    10-column layout — adds Prep.{Std,Perf,Imp}
    """
    print(header_comment(rows, tile_mode))
    has_prep = _has_any_prep(rows)
    for r in rows:
        p1_label = f"Tile {r['p1_tile']}"
        p2_label = f"Tile {r['p2_tile']}"
        prep_label = f"Tile {r['prep_tile']}" if r.get("prep_tile") is not None else "Tile ?"
        print()
        prep_note = f", Prep @ tile={r.get('prep_tile')}" if has_prep else ""
        print(f"% N={r['n']}: P1 @ tile={r['p1_tile']}, P2 @ tile={r['p2_tile']}{prep_note}")
        print(r"\begin{figure}[ht]")
        print(r"  \centering")
        if has_prep:
            print(r"\begin{tabular}{l@{\hspace{15pt}}rrr@{\hspace{15pt}}rrr@{\hspace{15pt}}rrr}")
            print(r"            & \multicolumn{3}{c}{P1}                          & \multicolumn{3}{c}{P2}                          & \multicolumn{3}{c}{Prep}                        \\")
            print(rf"            & Tile 1    & {p1_label:<9} & Imp.            & Tile 1    & {p2_label:<9} & Imp.            & Tile 1    & {prep_label:<9} & Imp.            \\")
            print(
                "Comm. (KB)  & {s1:>9} & {p1:>9} & {p1i}$\\times$ & {s2:>9} & {p2:>9} & {p2i}$\\times$ & {sp:>9} & {pp:>9} & {ppi}$\\times$ \\\\".format(
                    s1=fmt(r["p1_std_kb"], 1),
                    p1=fmt(r["p1_kb"], 1),
                    p1i=fmt(r["p1_imp_kb"], 2),
                    s2=fmt(r["p2_std_kb"], 1),
                    p2=fmt(r["p2_kb"], 1),
                    p2i=fmt(r["p2_imp_kb"], 2),
                    sp=fmt(r.get("prep_std_kb"), 1),
                    pp=fmt(r.get("prep_kb"), 1),
                    ppi=fmt(r.get("prep_imp_kb"), 2),
                )
            )
            print(
                "Time (ms)   & {s1:>9} & {p1:>9} & {p1i}$\\times$ & {s2:>9} & {p2:>9} & {p2i}$\\times$ & {sp:>9} & {pp:>9} & {ppi}$\\times$ \\\\".format(
                    s1=fmt(r["p1_std_ms"], 1),
                    p1=fmt(r["p1_ms"], 1),
                    p1i=fmt(r["p1_imp_ms"], 2),
                    s2=fmt(r["p2_std_ms"], 1),
                    p2=fmt(r["p2_ms"], 1),
                    p2i=fmt(r["p2_imp_ms"], 2),
                    sp=fmt(r.get("prep_std_ms"), 1),
                    pp=fmt(r.get("prep_ms"), 1),
                    ppi=fmt(r.get("prep_imp_ms"), 2),
                )
            )
        else:
            print(r"\begin{tabular}{l@{\hspace{15pt}}rrr@{\hspace{15pt}}rrr}")
            print(r"            & \multicolumn{3}{c}{P1}                          & \multicolumn{3}{c}{P2}                          \\")
            print(rf"            & Tile 1    & {p1_label:<9} & Imp.            & Tile 1    & {p2_label:<9} & Imp.            \\")
            print(
                "Comm. (KB)  & {s1:>9} & {p1:>9} & {p1i}$\\times$ & {s2:>9} & {p2:>9} & {p2i}$\\times$ \\\\".format(
                    s1=fmt(r["p1_std_kb"], 1),
                    p1=fmt(r["p1_kb"], 1),
                    p1i=fmt(r["p1_imp_kb"], 2),
                    s2=fmt(r["p2_std_kb"], 1),
                    p2=fmt(r["p2_kb"], 1),
                    p2i=fmt(r["p2_imp_kb"], 2),
                )
            )
            print(
                "Time (ms)   & {s1:>9} & {p1:>9} & {p1i}$\\times$ & {s2:>9} & {p2:>9} & {p2i}$\\times$ \\\\".format(
                    s1=fmt(r["p1_std_ms"], 1),
                    p1=fmt(r["p1_ms"], 1),
                    p1i=fmt(r["p1_imp_ms"], 2),
                    s2=fmt(r["p2_std_ms"], 1),
                    p2=fmt(r["p2_ms"], 1),
                    p2i=fmt(r["p2_imp_ms"], 2),
                )
            )
        print(r"\end{tabular}")
        print(r"% \caption{}")
        print(rf"  \label{{fig:table-{r['n']}}}")
        print(r"\end{figure}")


def emit_combined(rows, tile_mode: str) -> None:
    """One stacked table for all sizes.

    Without prep:  8-column layout — [N, label, P1.{Std,Perf,Imp}, P2.{Std,Perf,Imp}]
    With prep:    11-column layout — adds Prep.{Std,Perf,Imp}
    """
    print(header_comment(rows, tile_mode))
    has_prep = _has_any_prep(rows)
    p1_label = _perf_label(rows, "p1_tile")
    p2_label = _perf_label(rows, "p2_tile")
    print(r"\begin{figure}[ht]")
    print(r"  \centering")
    if has_prep:
        prep_label = _perf_label(rows, "prep_tile")
        print(r"\begin{tabular}{ll@{\hspace{15pt}}rrr@{\hspace{15pt}}rrr@{\hspace{15pt}}rrr}")
        print(r"           &              & \multicolumn{3}{c}{P1}                          & \multicolumn{3}{c}{P2}                          & \multicolumn{3}{c}{Prep}                        \\")
        print(rf"           &              & Tile 1    & {p1_label:<9} & Imp.            & Tile 1    & {p2_label:<9} & Imp.            & Tile 1    & {prep_label:<9} & Imp.            \\")
        for r in rows:
            print(r"\hline")
            print(
                "$N={n}$    & Comm. (KB)   & {s1:>9} & {p1:>9} & {p1i}$\\times$ & {s2:>9} & {p2:>9} & {p2i}$\\times$ & {sp:>9} & {pp:>9} & {ppi}$\\times$ \\\\".format(
                    n=r["n"],
                    s1=fmt(r["p1_std_kb"], 1),
                    p1=fmt(r["p1_kb"], 1),
                    p1i=fmt(r["p1_imp_kb"], 2),
                    s2=fmt(r["p2_std_kb"], 1),
                    p2=fmt(r["p2_kb"], 1),
                    p2i=fmt(r["p2_imp_kb"], 2),
                    sp=fmt(r.get("prep_std_kb"), 1),
                    pp=fmt(r.get("prep_kb"), 1),
                    ppi=fmt(r.get("prep_imp_kb"), 2),
                )
            )
            print(
                "           & Time (ms)    & {s1:>9} & {p1:>9} & {p1i}$\\times$ & {s2:>9} & {p2:>9} & {p2i}$\\times$ & {sp:>9} & {pp:>9} & {ppi}$\\times$ \\\\".format(
                    s1=fmt(r["p1_std_ms"], 1),
                    p1=fmt(r["p1_ms"], 1),
                    p1i=fmt(r["p1_imp_ms"], 2),
                    s2=fmt(r["p2_std_ms"], 1),
                    p2=fmt(r["p2_ms"], 1),
                    p2i=fmt(r["p2_imp_ms"], 2),
                    sp=fmt(r.get("prep_std_ms"), 1),
                    pp=fmt(r.get("prep_ms"), 1),
                    ppi=fmt(r.get("prep_imp_ms"), 2),
                )
            )
    else:
        print(r"\begin{tabular}{ll@{\hspace{15pt}}rrr@{\hspace{15pt}}rrr}")
        print(r"           &              & \multicolumn{3}{c}{P1}                          & \multicolumn{3}{c}{P2}                          \\")
        print(rf"           &              & Tile 1    & {p1_label:<9} & Imp.            & Tile 1    & {p2_label:<9} & Imp.            \\")
        for r in rows:
            print(r"\hline")
            print(
                "$N={n}$    & Comm. (KB)   & {s1:>9} & {p1:>9} & {p1i}$\\times$ & {s2:>9} & {p2:>9} & {p2i}$\\times$ \\\\".format(
                    n=r["n"],
                    s1=fmt(r["p1_std_kb"], 1),
                    p1=fmt(r["p1_kb"], 1),
                    p1i=fmt(r["p1_imp_kb"], 2),
                    s2=fmt(r["p2_std_kb"], 1),
                    p2=fmt(r["p2_kb"], 1),
                    p2i=fmt(r["p2_imp_kb"], 2),
                )
            )
            print(
                "           & Time (ms)    & {s1:>9} & {p1:>9} & {p1i}$\\times$ & {s2:>9} & {p2:>9} & {p2i}$\\times$ \\\\".format(
                    s1=fmt(r["p1_std_ms"], 1),
                    p1=fmt(r["p1_ms"], 1),
                    p1i=fmt(r["p1_imp_ms"], 2),
                    s2=fmt(r["p2_std_ms"], 1),
                    p2=fmt(r["p2_ms"], 1),
                    p2i=fmt(r["p2_imp_ms"], 2),
                )
            )
    print(r"\end{tabular}")
    print(r"% \caption{}")
    print(r"  \label{fig:table-combined}")
    print(r"\end{figure}")


def _maybe_fmt(x: Optional[float], digits: int) -> str:
    return fmt(x, digits) if x is not None else ""


def emit_csv(rows) -> None:
    print(
        "N,baseline_tile,p1_tile,p2_tile,prep_tile,"
        "p1_std_kb,p1_std_ms,p1_kb,p1_ms,p1_imp_kb,p1_imp_ms,"
        "p2_std_kb,p2_std_ms,p2_kb,p2_ms,p2_imp_kb,p2_imp_ms,"
        "prep_std_kb,prep_std_ms,prep_kb,prep_ms,prep_imp_kb,prep_imp_ms"
    )
    for r in rows:
        print(",".join([
            str(r["n"]),
            str(r["baseline_tile"]),
            str(r["p1_tile"]),
            str(r["p2_tile"]),
            str(r["prep_tile"]) if r.get("prep_tile") is not None else "",
            fmt(r["p1_std_kb"], 4),
            _maybe_fmt(r["p1_std_ms"], 4),
            fmt(r["p1_kb"], 4),
            _maybe_fmt(r["p1_ms"], 4),
            fmt(r["p1_imp_kb"], 4),
            _maybe_fmt(r["p1_imp_ms"], 4),
            fmt(r["p2_std_kb"], 4),
            _maybe_fmt(r["p2_std_ms"], 4),
            fmt(r["p2_kb"], 4),
            _maybe_fmt(r["p2_ms"], 4),
            fmt(r["p2_imp_kb"], 4),
            _maybe_fmt(r["p2_imp_ms"], 4),
            _maybe_fmt(r.get("prep_std_kb"), 4),
            _maybe_fmt(r.get("prep_std_ms"), 4),
            _maybe_fmt(r.get("prep_kb"), 4),
            _maybe_fmt(r.get("prep_ms"), 4),
            _maybe_fmt(r.get("prep_imp_kb"), 4),
            _maybe_fmt(r.get("prep_imp_ms"), 4),
        ]))


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    ap.add_argument("--log", type=Path, default=None,
                    help="bench log file (default: latest bench-*.log)")
    ap.add_argument("--criterion-root", type=Path,
                    default=Path("target/criterion/online"))
    ap.add_argument("--criterion-prep-root", type=Path,
                    default=Path("target/criterion/preprocessing"),
                    help="criterion output root for the preprocessing group")
    ap.add_argument("--sizes", default=",".join(str(s) for s in SIZES_DEFAULT),
                    help="comma-separated N values for square N×N cells "
                         f"(default: {','.join(str(s) for s in SIZES_DEFAULT)})")
    ap.add_argument("--tile-select", default="6",
                    help="tile reported under each protocol's Performance: "
                         "an integer (default '6' — paper's time/comm sweet-spot), "
                         "'comm' (argmin KB), or 'time' (argmin ms)")
    ap.add_argument("--format", choices=["separate", "combined", "csv"],
                    default="combined",
                    help="output layout (default: combined — paper's preferred "
                         "format for the experiments section)")
    ap.add_argument("--no-prep", action="store_true",
                    help="omit prep columns even when prep data is available")
    args = ap.parse_args()

    log_path = args.log or auto_detect_log()
    if log_path is None or not log_path.exists():
        print("error: no bench log found. pass --log <path>.", file=sys.stderr)
        return 2
    if not args.criterion_root.is_dir() and args.format != "csv":
        print(f"warning: {args.criterion_root} not found; ms columns will be '?'",
              file=sys.stderr)

    kb_data, _params = parse_kb(log_path)
    ms_data = parse_ms(args.criterion_root) if args.criterion_root.is_dir() else {}

    if args.no_prep:
        kb_prep, ms_prep = {}, {}
    else:
        kb_prep, _params_prep = parse_kb_prep(log_path)
        ms_prep = (
            parse_ms_prep(args.criterion_prep_root)
            if args.criterion_prep_root.is_dir()
            else {}
        )

    sizes = [int(s) for s in args.sizes.split(",") if s.strip()]

    rows = []
    for n in sizes:
        r = gather_row(n, kb_data, ms_data, args.tile_select,
                       kb_prep=kb_prep, ms_prep=ms_prep)
        if r is None:
            print(f"warning: no data for {n}x{n} (need both P1 and P2 cells)",
                  file=sys.stderr)
            continue
        rows.append(r)

    if not rows:
        print("error: no rows produced — check --log / --sizes / --tile-select",
              file=sys.stderr)
        return 2

    if args.format == "csv":
        emit_csv(rows)
    elif args.format == "combined":
        emit_combined(rows, args.tile_select)
    else:
        emit_separate(rows, args.tile_select)

    return 0


if __name__ == "__main__":
    sys.exit(main())
