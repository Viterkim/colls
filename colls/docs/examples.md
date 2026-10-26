# Examples

Anything before 0.1 can break the API.

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

The async is basically a compile time tag here, you can't await another `.with()` inside the closure while you're still holding the state. Do your async work before going in, the closure itself runs synchronously. Trying to grab the same state again from inside can deadlock or panic, just don't do that.

For several `CShared` values, make a group:

```rust
let other_state = CShared::new(State { score: 10 });
let group = CSharedGroup::new([&state, &other_state])?;

group.with(|[state, other]| state.score += other.score).await;
```

## Map

`CMap` is for shared entries you find by key.

```rust
let map = CMap::<String, u32>::new();
map.insert("score", 0).await?;

map.with("score", |v| *v += 1).await?;
```

A missing entry gets an error before the closure runs.

To get something back from the closure:

```rust
let score = map.with("score", |v| *v).await?;
```

For a few entries together:

```rust
map.insert("bonus", 10).await?;

map.with_many(["score", "bonus"], |[score, bonus]| {
    *score += *bonus;
    *bonus = 0;
}).await?;
```

The values come back in the order you pass their keys. Asking for the same entry twice gets rejected.

For repeated lookups, you can keep the key from `insert()` to skip hashing the name:

```rust
let score_key = map.insert("score", 0).await?;
map.with(&score_key, |v| *v += 1).await?;
```

Keys belong to the map that made them and still work after removing and reinserting the same name.
