# proj-lib-main-split

> Keep `main.rs` minimal and move logic into `lib.rs`.

## Why It Matters

Thin binaries make logic easier to test, reuse, and evolve without rewriting the entrypoint.

## Bad

```rust
fn main() {
    // Parsing, state management, IO, and formatting all mixed together.
}
```

## Good

```rust
// main.rs: parse args and call into the library.
fn main() {
    if let Err(err) = my_crate::run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
```

## See Also

- `proj-mod-by-feature.md`
- `proj-pub-crate-internal.md`
- `proj-prelude-module.md`
