# mem-zero-copy

> Prefer zero-copy slices and `Bytes` when possible.

## Why It Matters

Zero-copy means working with borrowed views into existing data instead of allocating and copying.
That reduces memory use and usually improves throughput.

## Bad

```rust
fn get_lines(data: &str) -> Vec<String> {
    data.lines().map(|line| line.to_string()).collect()
}
```

## Good

```rust
fn get_lines(data: &str) -> Vec<&str> {
    data.lines().collect()
}

fn process_packet(buffer: &[u8]) -> (&[u8], &[u8]) {
    (&buffer[0..16], &buffer[16..])
}
```

## When Borrowing Is Not Enough

```rust
use std::borrow::Cow;

fn normalize<'a>(input: &'a str) -> Cow<'a, str> {
    if input.contains('\t') {
        Cow::Owned(input.replace('\t', "    "))
    } else {
        Cow::Borrowed(input)
    }
}
```

## See Also

- `own-borrow-over-clone.md`
- `own-cow-conditional.md`
- `mem-avoid-format.md`
