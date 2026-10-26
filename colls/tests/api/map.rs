use super::run;
use colls::*;
use er::ErTest;
use std::{
    cell::Cell,
    hash::{Hash, Hasher},
    rc::Rc,
};

#[test]
fn entries() -> ErTest {
    let map = CMap::<String, String>::new();
    let alias = map.clone_ptr();
    let name = String::from("name");

    let key = run(map.insert(&name, String::from("cat")))?;

    run(map.with("name", |v| v.push('s')))?;
    run(alias.with(&key, |v| v.push('!')))?;

    let copied = key.clone_key();

    assert_eq!(run(map.remove(&name))?, "cats!");
    assert_eq!(
        run(map.with(&key, |_| panic!("missing key"))).unwrap_err(),
        CMapError::MissingKey
    );

    run(map.insert(&key, String::from("back")))?;

    assert_eq!(run(map.with(&copied, |v| v.clone()))?, "back");

    let expired = {
        let temporary = CMap::<String, u32>::new();
        run(temporary.insert("name", 1))?
    };

    let replacement = CMap::<String, u32>::new();

    assert_eq!(
        run(replacement.with(&expired, |_| panic!("expired key"))).unwrap_err(),
        CMapError::ForeignKey
    );

    Ok(())
}

#[derive(Clone)]
struct CountedKey {
    id: u32,
    hashes: Rc<Cell<usize>>,
}
impl PartialEq for CountedKey {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for CountedKey {}
impl Hash for CountedKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hashes.set(self.hashes.get() + 1);

        // Force collisions so the cached path must still compare keys.
        0_u32.hash(state);
    }
}

#[test]
fn cached() -> ErTest {
    let map = CMap::<CountedKey, u32>::new();
    let hashes = Rc::new(Cell::new(0));
    let key = run(map.insert(
        CountedKey {
            id: 0,
            hashes: Rc::clone(&hashes),
        },
        0,
    ))?;

    let mut keys = Vec::new();

    for id in 1..128 {
        keys.push(run(map.insert(
            CountedKey {
                id,
                hashes: Rc::new(Cell::new(0)),
            },
            id,
        ))?);
    }

    run(map.with(&key, |v| *v += 1))?;
    run(map.insert(&key, 5))?;
    run(map.with_many([&keys[3], &key, &keys[0]], |[v1, v2, v3]| {
        *v1 += 10;
        *v2 += 20;
        *v3 += 30;
    }))?;

    run(map.clone_inner(&key))?;
    run(map.remove(&key))?;

    assert_eq!(hashes.get(), 1);

    Ok(())
}

#[test]
fn group() -> ErTest {
    let map = CMap::<String, u32>::new();
    let k1 = run(map.insert("first", 10))?;
    let k2 = run(map.insert("second", 20))?;
    let copy = k1.clone_key();

    run(map.with_many(["second", "first"], |[v2, v1]| {
        *v2 -= 5;
        *v1 += 5;
    }))?;

    assert_eq!(run(map.clone_inner(&k1))?, 15);
    assert_eq!(run(map.clone_inner(&k2))?, 15);

    assert_eq!(
        run(map.with_many([&k1, &copy], |_| panic!("duplicate key"))),
        Err(CMapError::DuplicateKey)
    );

    run(map.remove(&k2))?;

    assert_eq!(
        run(map.with_many(["first", "second"], |_| panic!("missing key"))),
        Err(CMapError::MissingKey)
    );

    let other = CMap::<String, u32>::new();
    let foreign = run(other.insert("first", 99))?;

    assert_eq!(
        run(map.with_many([&k1, &foreign], |_| panic!("foreign key"))),
        Err(CMapError::ForeignKey)
    );

    Ok(())
}

#[test]
#[cfg(feature = "std")]
fn locking() {
    use std::sync::{Arc, Barrier};

    let map = CMap::<u32, u32>::new();
    let keys: Vec<_> = (0..64)
        .map(|key| run(map.insert(key, 0)).unwrap())
        .collect();
    let start = Arc::new(Barrier::new(3));

    let worker = |reverse| {
        let map = map.clone_ptr();
        let keys: Vec<_> = keys.iter().map(CKey::clone_key).collect();
        let start = Arc::clone(&start);

        std::thread::spawn(move || {
            let inputs: [_; 64] =
                std::array::from_fn(|index| &keys[if reverse { 63 - index } else { index }]);

            start.wait();

            for _ in 0..100 {
                run(map.with_many(inputs, |values| {
                    for v in values {
                        *v += 1;
                    }
                }))
                .unwrap();
            }
        })
    };

    let first = worker(false);
    let second = worker(true);

    start.wait();
    first.join().unwrap();
    second.join().unwrap();

    for key in &keys {
        assert_eq!(run(map.clone_inner(key)).unwrap(), 200);
    }
}
