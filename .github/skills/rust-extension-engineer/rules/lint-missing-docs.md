# lint-missing-docs

> Enable `missing_docs`.

## Why It Matters

Missing docs are easy to miss in review and expensive to fix later. Let the compiler catch them.

## Good

```rust
#![warn(missing_docs)]
```

## Also Useful

```toml
[workspace.lints.rust]
missing_docs = "warn"
```

## See Also

- `doc-all-public.md`
- `lint-workspace-lints.md`
- `lint-unsafe-doc.md`
