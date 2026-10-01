# colls

Use `.with()` when you need your state.

Anything up to 0.1 will not have a stable api.

## Shared

Put your state in `CShared` and pass handles around, then use `.with()` to get at the value.

```rust
use colls::*;

#[derive(Clone)]
pub struct State {
    pub score: u32,
}

let state = CShared::new(State { score: 0 });
state.with(|v| v.score += 1).await;

let other = state.clone_ptr();
let copy = state.clone_inner().await;
```

`other` uses the same state, `copy` is a copy of the value inside.

The async is basically a compile time tag here, you can't await another `.with()` inside the closure while you're still holding the state. Do your async work before going in, the closure itself runs synchronously. Manually polling a future can get around it and deadlock you, just don't do that, same as putting `std::thread::sleep()` in an async function.

Keep it small, waiting for the default lock blocks the thread.

## Map

`CMap` is for shared entries you find by key.

```rust
let map = CMap::<String, u32>::new();
map.insert("score", 0).await?;

let score_key = map.with("score", |v| *v += 1).await?;
map.with(&score_key, |v| *v += 1).await?;
```

Keep the key to skip hashing it next time, otherwise just ignore it. `insert()` returns one too. Keys belong to the map that made them and still work after removing and reinserting the same name. A missing entry gets an error before the closure runs.

To get something back from the closure:

```rust
let score = map.with_many([&score_key], |[v]| *v).await?;
```

For a few entries together:

```rust
let bonus_key = map.insert("bonus", 10).await?;

map.with_many([&score_key, &bonus_key], |[score, bonus]| {
    *score += *bonus;
    *bonus = 0;
}).await?;
```

The values come back in the order you pass their keys. Asking for the same entry twice gets rejected.

For several `CShared` values, make a group:

```rust
let other_state = CShared::new(State { score: 10 });
let group = CSharedGroup::new([&state, &other_state])?;

group.with(|[state, other]| state.score += other.score).await;
```
