# mem-with-capacity

> Use `with_capacity()` when you know the size.

## Why It Matters

Pre-allocating avoids repeated growth and reallocation during push-heavy loops.

## Bad

```rust
fn collect_paths(entries: &[Entry]) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for entry in entries {
        paths.push(entry.path.clone());
    }
    paths
}
```

## Good

```rust
fn collect_paths(entries: &[Entry]) -> Vec<PathBuf> {
    let mut paths = Vec::with_capacity(entries.len());
    for entry in entries {
        paths.push(entry.path.clone());
    }
    paths
}
```

## Also Useful For

```rust
let mut s = String::with_capacity(256);
let mut map = HashMap::with_capacity(expected_items);
```

## See Also

- `mem-reuse-collections.md`
- `mem-clone-from.md`
- `perf-entry-api.md`
