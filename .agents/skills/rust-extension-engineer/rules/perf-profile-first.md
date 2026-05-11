# perf-profile-first

> Profile before optimizing.

## Why It Matters

The thing you suspect is slow often is not the real bottleneck. Profile the actual workload before
changing code.

## Bad

```rust
fn process(data: &[Item]) -> Vec<Output> {
    let cloned: Vec<_> = data.iter().cloned().collect();
    cloned.iter().map(expensive_computation).collect()
}
```

## Good

```rust
// 1. Measure with a flamegraph or profiler.
// 2. Find the actual hot path.
// 3. Optimize only that path.
```

## Common Tools

- `cargo flamegraph`
- `perf`
- Instruments on macOS
- `criterion` for micro-benchmarks

## See Also

- `perf-black-box-bench.md`
- `perf-release-profile.md`
- `anti-premature-optimize.md`
