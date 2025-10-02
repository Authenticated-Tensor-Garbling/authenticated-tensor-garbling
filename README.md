# Authenticated Tensor Garbling

This repository contains the implementation and benchmarks for authenticated tensor garbling protocols.

## Running Benchmarks

To run the benchmarks, use:

```bash
cargo bench
```

This will execute all benchmark suites including:
- Full protocol garbling benchmarks
- Runtime benchmarks with network simulation
- Performance analysis across different matrix sizes and chunking factors

## Project Structure

- `src/` - Core implementation of semihonest and authenticated garbled tensor protocols
- `benches/` - Benchmark suites and network simulation