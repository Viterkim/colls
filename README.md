# Colls, collections of abstractions / collections for concurrency

Use `.with()` when you need your state.

```toml
[dependencies]
colls = "0.0.2"
```

Anything before 0.1 can break the API.

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

The async is basically a compile time tag here, you can't await another `.with()` inside the closure while you're still holding the state. Do your async work before going in, the closure itself runs synchronously. Trying to grab the same state again from inside can deadlock or panic, just don't do that.

## Map

`CMap` is for shared entries you find by key.

```rust
let map = CMap::<String, u32>::new();
map.insert("score", 0).await?;

map.with("score", |v| *v += 1).await?;

let score = map.with("score", |v| *v).await?;
```

## Links

[Examples](colls/docs/examples.md)

[Platform setup](colls/docs/platforms.md)

[GitHub Repo](https://github.com/Viterkim/colls)

[Docs.rs](https://docs.rs/colls/latest/colls/)

[Crates.io for the lib](https://crates.io/crates/colls)

[Libs.rs](https://lib.rs/crates/colls)
