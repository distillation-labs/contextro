# test-proptest-properties

> Use `proptest` for property-based testing.

## Why It Matters

Property-based testing explores edge cases humans usually miss, and it shrinks failures to smaller
counterexamples.

## Example

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn reverse_reverse_is_identity(s in ".*") {
        let reversed: String = s.chars().rev().collect();
        let double_reversed: String = reversed.chars().rev().collect();
        prop_assert_eq!(s, double_reversed);
    }
}
```

## Custom Strategy

```rust
#[derive(Debug, Clone)]
struct User {
    name: String,
    age: u8,
}

fn user_strategy() -> impl Strategy<Value = User> {
    ("[a-zA-Z]{1,20}", 0..120u8).prop_map(|(name, age)| User { name, age })
}
```

## See Also

- `test-arrange-act-assert.md`
- `test-doctest-examples.md`
- `test-criterion-bench.md`
