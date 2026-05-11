# async-no-lock-await

> Never hold `Mutex` or `RwLock` across `.await`.

## Why It Matters

Holding a lock across an await point can deadlock or starve other tasks. The task may suspend
while still owning shared state.

## Bad

```rust
use tokio::sync::Mutex;

async fn bad_update(state: &Mutex<State>) {
    let mut guard = state.lock().await;
    let data = fetch_from_network().await;
    guard.value = data;
}
```

## Good

```rust
use tokio::sync::Mutex;

async fn good_update(state: &Mutex<State>) {
    let data = fetch_from_network().await;
    let mut guard = state.lock().await;
    guard.value = data;
}
```

## Common Fixes

```rust
// Clone the data you need, drop the lock, then await.
let id = {
    let guard = state.lock().await;
    guard.id.clone()
};

let data = fetch_by_id(id).await;
```

## See Also

- `async-clone-before-await.md`
- `async-spawn-blocking.md`
- `anti-lock-across-await.md`
