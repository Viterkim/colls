# Colls, collections of abstractions / collections for concurrency

Use `.with()` when you need your state.

```toml
[dependencies]
colls = "0.0.1"
```

Anything up to 0.1 will not have a stable api.

## Shared

Put your state in `CShared` and pass handles around, then use `.with()` to get at the value.

```rust
use colls::*;

pub struct State {
    pub score: u32,
}

let state = CShared::new(State { score: 0 });
state.with(|v| v.score += 1).await;
```

The async is basically a compile time tag here, you can't await another `.with()` inside the closure while you're still holding the state. Do your async work before going in, the closure itself runs synchronously. Manually polling a future can get around it and deadlock you, just don't do that, same as putting `std::thread::sleep()` in an async function.

Keep it small, use it to just update values/state.

## Map

`CMap` is for shared entries you find by key.

```rust
let map = CMap::<String, u32>::new();
map.insert("score", 0).await?;

let key = map.with("score", |v| *v += 1).await?;
map.with(&key, |v| *v += 1).await?;

let score = map.with_many([&key], |[v]| *v).await?;
```

Keep the key to skip hashing it next time, or just ignore it. The value stays inside the closure. `with_many()` gives you the closure's return.

## Docs

[Examples](colls/docs/examples.md)

[Platform setup](colls/docs/platforms.md)
