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

Subcommands:

  - `vertical [--n 256]`: single-N transposed table (Π-row × metric-col ×
    {Tile 1, Tile k, Improvement}). Drops into the paper's main section.
    Writes `<out>/comparison_table_vertical.tex`.
  - `horizontal [--sizes 64,128,256]`: multi-N table comparing tile k vs
    tile 1 across N's. Drops into the paper's appendix as a `figure*`
    (two-column-spanning). Writes `<out>/comparison_table_horizontal.tex`.
  - `csv [--sizes 64,128,256]`: raw numbers for spreadsheet / sanity-check
    use. Writes `<out>/comparison_table.csv`.

Usage:
    python3 tools/comparison_table.py vertical                              # N=256
    python3 tools/comparison_table.py horizontal --sizes 64,128,256
    python3 tools/comparison_table.py csv --sizes 64,128,256
    python3 tools/comparison_table.py vertical --tile-select comm           # argmin-by-comm

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
    """Plain decimal format, no thousands separator."""
    return f"{x:.{digits}f}" if x is not None else "?"


def fmt_grouped(x: Optional[float], digits: int) -> str:
    """Decimal format with comma thousands separator (for LaTeX tables)."""
    return f"{x:,.{digits}f}" if x is not None else "?"


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


# ---------------------------------------------------------------------------
# Vertical (single-N) and horizontal (multi-N) emitters
# ---------------------------------------------------------------------------


VERTICAL_CAPTION = (
    "Tile 1 vs. tile 6 comparison for $\\twopct$, $\\twopctauth$, "
    "and uncompressed preprocessing. All three comparisons show "
    "communication improvement near-linear in the tile size, with "
    "a corresponding wall-clock speedup."
)

HORIZONTAL_CAPTION = (
    "Tile 6 vs. tile 1 at $N \\in \\{{{ns}\\}}$ for $\\twopct$, "
    "$\\twopctauth$, and uncompressed preprocessing. Both metric blocks "
    "share a column structure; values are absolute, with the per-protocol "
    "improvement ratio."
)


def emit_vertical(row, tile_mode: str, has_prep: bool, out_path: Path) -> None:
    """Single-N transposed table — Π-row × {Tile 1, Tile k, Improvement}
    sub-rows × {Comm. (KB), Time (ms)} columns. Uses booktabs rules and
    `\\twopct` / `\\twopctauth` macros (defined by the paper). Comma
    thousands separator on values >999. Each protocol block is followed
    by `\\midrule`.

    The prep block is omitted entirely when `has_prep` is false (rather
    than emitting `?` rows) — keeps the table clean on partial bench runs.
    """
    n = row["n"]
    p1_tile = row["p1_tile"]
    p2_tile = row["p2_tile"]
    prep_tile = row.get("prep_tile") if has_prep else None

    lines: list[str] = []
    lines.append(header_comment([row], tile_mode))
    lines.append(r"\begin{figure}[ht]")
    lines.append(r"  \centering")
    lines.append(r"\begin{tabular}{llrr}")
    lines.append(r"           &              & Comm. (KB) & Time (ms) \\")
    lines.append(r"\toprule")

    def emit_block(label: str, baseline_tile: int, perf_tile: int,
                   std_kb, perf_kb, imp_kb, std_ms, perf_ms, imp_ms,
                   trailing_rule: str) -> None:
        lines.append(
            f"{label} & Tile {baseline_tile}     & {fmt_grouped(std_kb, 1):>10}  & {fmt_grouped(std_ms, 1):>10} \\\\"
        )
        lines.append(
            f"           & Tile {perf_tile}     & {fmt_grouped(perf_kb, 1):>10}  & {fmt_grouped(perf_ms, 1):>10} \\\\"
        )
        lines.append(
            f"           & \\textit{{Improvement}} & {fmt(imp_kb, 2)}$\\times$ & {fmt(imp_ms, 2)}$\\times$ \\\\"
        )
        if trailing_rule:
            lines.append(trailing_rule)

    emit_block(
        r"$\twopct$",
        BASELINE_TILE, p1_tile,
        row["p1_std_kb"], row["p1_kb"], row["p1_imp_kb"],
        row["p1_std_ms"], row["p1_ms"], row["p1_imp_ms"],
        r"\midrule",
    )
    emit_block(
        r"$\twopctauth$",
        BASELINE_TILE, p2_tile,
        row["p2_std_kb"], row["p2_kb"], row["p2_imp_kb"],
        row["p2_std_ms"], row["p2_ms"], row["p2_imp_ms"],
        r"\midrule" if has_prep else r"\bottomrule",
    )
    if has_prep:
        emit_block(
            r"Uncomp. Pre.",
            BASELINE_TILE, prep_tile,
            row["prep_std_kb"], row["prep_kb"], row["prep_imp_kb"],
            row["prep_std_ms"], row["prep_ms"], row["prep_imp_ms"],
            r"\bottomrule",
        )

    lines.append(r"\end{tabular}")
    lines.append(rf"\caption{{{VERTICAL_CAPTION}}}")
    lines.append(rf"  \label{{fig:table-vertical-{n}}}")
    lines.append(r"\end{figure}")

    out_path.write_text("\n".join(lines) + "\n")
    print(f"  wrote {out_path}")


def emit_horizontal(rows, tile_mode: str, has_prep: bool, out_path: Path) -> None:
    """Multi-N table — N-rows × {P1, P2, Prep} × {Tile 1, Tile k, Imp}
    columns, with Comm. and Time. blocks stacked under a single column
    structure (intra-block separator via `\\multicolumn{10}{l}{\\textit{...}}`).

    Renders as `figure*` (two-column-spanning) since the 10-column layout
    is too wide for a single column. Uses `cmidrule(lr)` rather than
    `cline` for booktabs consistency.
    """
    if has_prep:
        col_spec = "lrrrrrrrrr"      # N + 3 protocols × 3 cols
        protocol_groups = (
            r" & \multicolumn{3}{c}{$\twopct$}"
            r" & \multicolumn{3}{c}{$\twopctauth$}"
            r" & \multicolumn{3}{c}{Uncomp. Pre.} \\"
        )
        midrules = r"\cmidrule(lr){2-4} \cmidrule(lr){5-7} \cmidrule(lr){8-10}"
        sub_header = (
            f"$N$ & Tile {BASELINE_TILE} & Tile {{p1}} & Imp."
            f" & Tile {BASELINE_TILE} & Tile {{p2}} & Imp."
            f" & Tile {BASELINE_TILE} & Tile {{pp}} & Imp. \\\\"
        )
        n_cols = 10
    else:
        col_spec = "lrrrrrr"
        protocol_groups = (
            r" & \multicolumn{3}{c}{$\twopct$}"
            r" & \multicolumn{3}{c}{$\twopctauth$} \\"
        )
        midrules = r"\cmidrule(lr){2-4} \cmidrule(lr){5-7}"
        sub_header = (
            f"$N$ & Tile {BASELINE_TILE} & Tile {{p1}} & Imp."
            f" & Tile {BASELINE_TILE} & Tile {{p2}} & Imp. \\\\"
        )
        n_cols = 7

    # Use the first row's tile picks for the column header. _perf_label
    # already handles the "all rows agree" → "Tile k" / mixed → "Best tile"
    # convention; we strip "Tile " for the format slot.
    p1_label = _perf_label(rows, "p1_tile").replace("Tile ", "")
    p2_label = _perf_label(rows, "p2_tile").replace("Tile ", "")
    pp_label = _perf_label(rows, "prep_tile").replace("Tile ", "") if has_prep else ""
    sub_header_filled = sub_header.format(p1=p1_label, p2=p2_label, pp=pp_label)

    def emit_metric_block(metric_label: str, suffix: str, digits: int) -> list[str]:
        """One metric block (Comm. or Time.) — header row + one row per N.

        `suffix` is "kb" or "ms"; row keys are `{proto}_std_{suffix}`,
        `{proto}_{suffix}`, `{proto}_imp_{suffix}`.
        """
        block: list[str] = [
            rf"\multicolumn{{{n_cols}}}{{l}}{{\textit{{{metric_label}}}}} \\"
        ]
        for r in rows:
            cells: list[str] = [str(r["n"])]
            cells += [
                fmt_grouped(r[f"p1_std_{suffix}"], digits),
                fmt_grouped(r[f"p1_{suffix}"], digits),
                f"{fmt(r[f'p1_imp_{suffix}'], 2)}$\\times$",
                fmt_grouped(r[f"p2_std_{suffix}"], digits),
                fmt_grouped(r[f"p2_{suffix}"], digits),
                f"{fmt(r[f'p2_imp_{suffix}'], 2)}$\\times$",
            ]
            if has_prep:
                cells += [
                    fmt_grouped(r.get(f"prep_std_{suffix}"), digits),
                    fmt_grouped(r.get(f"prep_{suffix}"), digits),
                    f"{fmt(r.get(f'prep_imp_{suffix}'), 2)}$\\times$",
                ]
            block.append(" & ".join(cells) + r" \\")
        return block

    lines: list[str] = []
    lines.append(header_comment(rows, tile_mode))
    lines.append(r"\begin{figure*}[ht]")
    lines.append(r"  \centering")
    lines.append(rf"\begin{{tabular}}{{{col_spec}}}")
    lines.append(r"\toprule")
    lines.append(protocol_groups)
    lines.append(midrules)
    lines.append(sub_header_filled)
    lines.append(r"\midrule")
    lines.extend(emit_metric_block("Communication (KB)", "kb", 1))
    lines.append(r"\midrule")
    lines.extend(emit_metric_block("Time (ms)", "ms", 1))
    lines.append(r"\bottomrule")
    lines.append(r"\end{tabular}")
    ns_str = ", ".join(str(r["n"]) for r in rows)
    lines.append(rf"\caption{{{HORIZONTAL_CAPTION.format(ns=ns_str)}}}")
    lines.append(r"\label{fig:table-horizontal}")
    lines.append(r"\end{figure*}")

    out_path.write_text("\n".join(lines) + "\n")
    print(f"  wrote {out_path}")


def _maybe_fmt(x: Optional[float], digits: int) -> str:
    return fmt(x, digits) if x is not None else ""


CSV_HEADER = (
    "N,baseline_tile,p1_tile,p2_tile,prep_tile,"
    "p1_std_kb,p1_std_ms,p1_kb,p1_ms,p1_imp_kb,p1_imp_ms,"
    "p2_std_kb,p2_std_ms,p2_kb,p2_ms,p2_imp_kb,p2_imp_ms,"
    "prep_std_kb,prep_std_ms,prep_kb,prep_ms,prep_imp_kb,prep_imp_ms"
)


def csv_lines(rows) -> list[str]:
    out = [CSV_HEADER]
    for r in rows:
        out.append(",".join([
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
    return out


# ---------------------------------------------------------------------------
# Subcommand plumbing
# ---------------------------------------------------------------------------


def load_data(args, *, need_criterion: bool):
    log_path = args.log or auto_detect_log()
    if log_path is None or not log_path.exists():
        print("error: no bench log found. pass --log <path>.", file=sys.stderr)
        return None
    if need_criterion and not args.criterion_root.is_dir():
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

    return kb_data, ms_data, kb_prep, ms_prep


def collect_rows(args, kb_data, ms_data, kb_prep, ms_prep, sizes: list[int]):
    rows = []
    for n in sizes:
        r = gather_row(n, kb_data, ms_data, args.tile_select,
                       kb_prep=kb_prep, ms_prep=ms_prep)
        if r is None:
            print(f"warning: no data for {n}x{n} (need both P1 and P2 cells)",
                  file=sys.stderr)
            continue
        rows.append(r)
    return rows


def cmd_vertical(args) -> int:
    """Single-N transposed table → <out>/comparison_table_vertical.tex."""
    data = load_data(args, need_criterion=True)
    if data is None:
        return 2
    kb_data, ms_data, kb_prep, ms_prep = data
    rows = collect_rows(args, kb_data, ms_data, kb_prep, ms_prep, [args.n])
    if not rows:
        print(f"error: no row produced for N={args.n}", file=sys.stderr)
        return 2
    has_prep = _has_any_prep(rows)
    args.out.mkdir(parents=True, exist_ok=True)
    out_path = args.out / "comparison_table_vertical.tex"
    emit_vertical(rows[0], args.tile_select, has_prep, out_path)
    return 0


def cmd_horizontal(args) -> int:
    """Multi-N table → <out>/comparison_table_horizontal.tex."""
    data = load_data(args, need_criterion=True)
    if data is None:
        return 2
    kb_data, ms_data, kb_prep, ms_prep = data
    sizes = [int(s) for s in args.sizes.split(",") if s.strip()]
    rows = collect_rows(args, kb_data, ms_data, kb_prep, ms_prep, sizes)
    if not rows:
        print("error: no rows produced — check --log / --sizes / --tile-select",
              file=sys.stderr)
        return 2
    has_prep = _has_any_prep(rows)
    args.out.mkdir(parents=True, exist_ok=True)
    out_path = args.out / "comparison_table_horizontal.tex"
    emit_horizontal(rows, args.tile_select, has_prep, out_path)
    return 0


def cmd_csv(args) -> int:
    """CSV → <out>/comparison_table.csv (covers --sizes for cross-check)."""
    data = load_data(args, need_criterion=False)
    if data is None:
        return 2
    kb_data, ms_data, kb_prep, ms_prep = data
    sizes = [int(s) for s in args.sizes.split(",") if s.strip()]
    rows = collect_rows(args, kb_data, ms_data, kb_prep, ms_prep, sizes)
    if not rows:
        print("error: no rows produced — check --log / --sizes / --tile-select",
              file=sys.stderr)
        return 2
    args.out.mkdir(parents=True, exist_ok=True)
    out_path = args.out / "comparison_table.csv"
    out_path.write_text("\n".join(csv_lines(rows)) + "\n")
    print(f"  wrote {out_path}")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )

    def add_shared(sub):
        sub.add_argument("--log", type=Path, default=None,
                         help="bench log file (default: latest bench-*.log)")
        sub.add_argument("--criterion-root", type=Path,
                         default=Path("target/criterion"),
                         help="criterion output root (default: target/criterion)")
        sub.add_argument("--criterion-prep-root", type=Path,
                         default=Path("target/criterion/preprocessing"),
                         help="criterion output root for the preprocessing group")
        sub.add_argument("--out", type=Path, default=Path("figures"),
                         help="output directory (default: figures/)")
        sub.add_argument("--tile-select", default="6",
                         help="performance tile: integer (default '6'), "
                              "'comm' (argmin KB), or 'time' (argmin ms)")
        sub.add_argument("--no-prep", action="store_true",
                         help="omit prep columns even when prep data is available")

    sub = ap.add_subparsers(dest="cmd", required=True, metavar="<command>")

    p_vert = sub.add_parser("vertical",
                            help="single-N transposed table (paper main section)")
    add_shared(p_vert)
    p_vert.add_argument("--n", type=int, default=256,
                        help="square size N×N (default: 256)")
    p_vert.set_defaults(func=cmd_vertical)

    p_horz = sub.add_parser("horizontal",
                            help="multi-N tile-1-vs-tile-k table (paper appendix)")
    add_shared(p_horz)
    p_horz.add_argument("--sizes",
                        default=",".join(str(s) for s in SIZES_DEFAULT),
                        help=f"comma-separated N values "
                             f"(default: {','.join(str(s) for s in SIZES_DEFAULT)})")
    p_horz.set_defaults(func=cmd_horizontal)

    p_csv = sub.add_parser("csv", help="raw numbers as CSV")
    add_shared(p_csv)
    p_csv.add_argument("--sizes",
                       default=",".join(str(s) for s in SIZES_DEFAULT),
                       help=f"comma-separated N values "
                            f"(default: {','.join(str(s) for s in SIZES_DEFAULT)})")
    p_csv.set_defaults(func=cmd_csv)

    args = ap.parse_args()
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
