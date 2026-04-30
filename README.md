# Authenticated Tensor Garbling

Implementation and benchmarks for authenticated tensor garbling protocols.

## Tests

```bash
cargo test --lib
```

## Benchmarks

```bash
RUSTFLAGS="-C target-cpu=native" cargo bench --features bench-internals
```
Sweeps `(n, m) ∈ BENCHMARK_PARAMS × chunking_factor 1..=8` for both online
protocols and uncompressed preprocessing under a 100 Mbps network model.

```bash
RUSTFLAGS="-C target-cpu=native" cargo bench --features bench-internals 2>&1 \
  | tee bench-$(date +%Y%m%d-%H%M).log
```
Collects the emitted networking analysis for use in plotting.
Each cell emits a `KB,…` log line; per-cell timing lives in
`target/criterion/`.

## Figures and tables

Requires `matplotlib`:

```bash
python3 -m venv tools/.venv
tools/.venv/bin/pip install matplotlib
```

Then, against the latest `bench-*.log` (auto-detected):

```bash
tools/.venv/bin/python3 tools/parse_results.py main          # 6 PDFs → figures/main/ (N=256)
tools/.venv/bin/python3 tools/parse_results.py appendix      # 18 PDFs → figures/appendix/ (N ∈ {64,128,256})
tools/.venv/bin/python3 tools/comparison_table.py vertical   # main-paper table
tools/.venv/bin/python3 tools/comparison_table.py horizontal # appendix table
```

## Project structure

- `src/` — protocol implementation
- `benches/` — Criterion benchmark suites and network simulator
- `tests/` — integration tests
- `tools/` — figure and table generators (Python)
