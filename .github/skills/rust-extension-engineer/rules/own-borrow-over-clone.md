# own-borrow-over-clone

> Prefer `&T` borrowing over `.clone()`.

## Why It Matters

Cloning allocates and copies data. Borrowing is usually free, keeps APIs more flexible, and
reduces pressure on allocators and caches.

## Bad

```rust
fn process(data: &String) {
    let local = data.clone();
    println!("{}", local);
}

fn process_all(items: &[String]) {
    for item in items {
        let copy = item.clone();
        handle(&copy);
    }
}
```

## Good

```rust
fn process(data: &str) {
    println!("{}", data);
}

fn process_all(items: &[String]) {
    for item in items {
        handle(item);
    }
}
```

## When Clone Is Acceptable

```rust
// Owning data for storage is a real reason to clone.
struct Cache {
    data: std::collections::HashMap<String, String>,
}

impl Cache {
    fn insert(&mut self, key: &str, value: &str) {
        self.data.insert(key.to_string(), value.to_string());
    }
}

// Copy types do not allocate.
let x: i32 = 42;
let y = x;
```

## See Also

- `own-slice-over-vec.md`
- `own-cow-conditional.md`
- `mem-clone-from.md`
