# own-slice-over-vec

> Prefer `&[T]` and `&str` over `&Vec<T>` and `&String`.

## Why It Matters

Accepting a concrete collection type overconstrains callers. Slice-based APIs accept vectors,
arrays, and borrowed data without forcing an allocation or an owned wrapper.

## Bad

```rust
fn count_words(text: &String) -> usize {
    text.split_whitespace().count()
}

fn sum(values: &Vec<i32>) -> i32 {
    values.iter().sum()
}
```

## Good

```rust
fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

fn sum(values: &[i32]) -> i32 {
    values.iter().sum()
}
```

## When A Collection Reference Is Fine

```rust
fn mutate(values: &mut [i32]) {
    for value in values {
        *value += 1;
    }
}
```

## See Also

- `own-borrow-over-clone.md`
- `own-cow-conditional.md`
- `type-no-stringly.md`
