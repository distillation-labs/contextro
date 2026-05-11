# async-spawn-blocking

> Use `spawn_blocking` for CPU-intensive work.

## Why It Matters

CPU-bound work blocks the async runtime and delays unrelated tasks. Offload it so the executor
can keep servicing I/O.

## Bad

```rust
async fn hash_all(files: Vec<PathBuf>) -> Vec<Hash> {
    files.into_iter().map(|path| expensive_hash(&path)).collect()
}
```

## Good

```rust
async fn hash_all(files: Vec<PathBuf>) -> Vec<Hash> {
    tokio::task::spawn_blocking(move || {
        files.into_iter().map(|path| expensive_hash(&path)).collect()
    })
    .await
    .expect("hashing task panicked")
}
```

## See Also

- `async-no-lock-await.md`
- `perf-profile-first.md`
- `opt-cold-unlikely.md`
