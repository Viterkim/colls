use colls::*;
use er::ErTest;
use std::{
    cell::Cell,
    future::Future,
    hash::{Hash, Hasher},
    panic::{AssertUnwindSafe, catch_unwind},
    pin::pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

fn run<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());

    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("colls operations should finish on their first poll"),
    }
}

#[test]
fn value() {
    let value = CShared::new(String::from("cat"));
    let alias = value.clone_ptr();
    let snapshot = run(value.clone_inner());

    run(alias.with(|v| v.push('s')));
    assert_eq!(run(value.clone_inner()), "cats");
    assert_eq!(snapshot, "cat");
    assert!(value.is_same_ptr(&alias));
    assert_eq!(run(CShared::<u32>::default().with(|v| *v)), 0);

    let local = CShared::new(Rc::new(Cell::new(1)));
    run(local.with(|v| v.set(2)));
    assert_eq!(run(local.with(|v| v.get())), 2);

    let failure: Result<(), ()> = run(value.with(|v| {
        v.push('!');
        Err(())
    }));
    assert!(failure.is_err());
    assert_eq!(run(value.clone_inner()), "cats!");
}

#[test]
fn group() -> ErTest {
    let first = CShared::new(String::from("first"));
    let second = CShared::new(String::from("second"));
    let third = CShared::new(String::from("third"));
    let alias = first.clone_ptr();

    let error = CSharedGroup::new([&first, &alias]).err().unwrap();

    assert_eq!(
        error.to_string(),
        "the same CShared value was passed more than once"
    );

    let group = CSharedGroup::new([&third, &first, &second])?;
    run(group.with(|[v3, v1, v2]| {
        v3.push_str("-3");
        v1.push_str("-1");
        v2.push_str("-2");
    }));

    assert_eq!(run(first.clone_inner()), "first-1");
    assert_eq!(run(second.clone_inner()), "second-2");
    assert_eq!(run(third.clone_inner()), "third-3");

    let single = CSharedGroup::new([&third])?;

    assert_eq!(run(single.with(|[v]| v.len())), 7);

    let empty = CSharedGroup::<u32, 0>::new([])?;

    assert_eq!(run(empty.with(|[]| 42)), 42);

    Ok(())
}

#[test]
#[cfg(feature = "std")]
fn shared_locking() {
    use std::sync::{Arc, Barrier};

    let first = CShared::new(0);
    let second = CShared::new(0);
    let start = Arc::new(Barrier::new(3));

    let worker = |reverse| {
        let first = first.clone_ptr();
        let second = second.clone_ptr();
        let start = Arc::clone(&start);

        std::thread::spawn(move || {
            let inputs = if reverse {
                [&second, &first]
            } else {
                [&first, &second]
            };

            let group = CSharedGroup::new(inputs).unwrap();
            start.wait();

            for _ in 0..1000 {
                run(group.with(|[v1, v2]| {
                    *v1 += 1;
                    *v2 += 1;
                }));
            }
        })
    };

    let left = worker(false);
    let right = worker(true);
    start.wait();
    left.join().unwrap();
    right.join().unwrap();

    assert_eq!(run(first.clone_inner()), 2000);
    assert_eq!(run(second.clone_inner()), 2000);
}

#[test]
fn map() -> ErTest {
    let map = CMap::<String, String>::new();
    let alias = map.clone_ptr();
    let key = run(map.insert("name", String::from("cat")))?;
    let snapshot = run(map.clone_inner(&key))?;

    let key = run(map.with("name", |v| v.push('s')))?;
    let borrowed = run(alias.with(&key, |v| v.push('!')))?;

    assert!(std::ptr::eq(borrowed, &key));
    assert_eq!(run(map.clone_inner(&key))?, "cats!");
    assert_eq!(snapshot, "cat");

    let copied = key.clone_key();

    assert_eq!(run(alias.clone_inner(&copied))?, "cats!");

    let other = CMap::<String, String>::new();

    assert_eq!(
        run(other.with(&key, |_| panic!("foreign key"))).unwrap_err(),
        CMapError::ForeignKey
    );

    assert_eq!(run(map.remove(&key))?, "cats!");
    assert_eq!(
        run(map.with(&key, |_| panic!("missing key"))).unwrap_err(),
        CMapError::MissingKey
    );
    run(map.insert(&key, String::from("back")))?;
    assert_eq!(run(map.clone_inner(&copied))?, "back");

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
    assert_eq!(hashes.get(), 1);

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

    assert_eq!(run(map.clone_inner(&key))?, 25);
    assert_eq!(run(map.clone_inner(&keys[3]))?, 14);
    assert_eq!(run(map.clone_inner(&keys[0]))?, 31);
    assert_eq!(run(map.remove(&key))?, 25);
    assert_eq!(hashes.get(), 1);

    Ok(())
}

#[test]
fn map_group() -> ErTest {
    let map = CMap::<u32, u32>::new();
    let k1 = run(map.insert(1, 10))?;
    let k2 = run(map.insert(2, 20))?;
    let copy = k1.clone_key();

    assert_eq!(run(map.with_many([], |[]| 42))?, 42);
    assert_eq!(run(map.with_many([&k1], |[v]| *v))?, 10);

    run(map.with_many([&k2, &k1], |[v2, v1]| {
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
        run(map.with_many([&k1, &k2], |_| panic!("missing key"))),
        Err(CMapError::MissingKey)
    );
    assert_eq!(run(map.clone_inner(&k1))?, 15);

    let other = CMap::<u32, u32>::new();
    let foreign = run(other.insert(1, 99))?;

    assert_eq!(
        run(map.with_many([&k1, &foreign], |_| panic!("foreign key"))),
        Err(CMapError::ForeignKey)
    );

    Ok(())
}

#[test]
fn unwind() {
    let first = CShared::new(0);
    let second = CShared::new(0);

    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            run(first.with(|v| {
                *v += 1;
                panic!("stop");
            }));
        }))
        .is_err()
    );
    assert_eq!(run(first.with(|v| *v)), 1);

    let group = CSharedGroup::new([&first, &second]).unwrap();

    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            run(group.with(|[v1, v2]| {
                *v1 += 1;
                *v2 += 1;
                panic!("stop");
            }));
        }))
        .is_err()
    );
    assert_eq!(run(group.with(|[v1, v2]| (*v1, *v2))), (2, 1));

    let map = CMap::<u32, u32>::new();
    let k1 = run(map.insert(1, 0)).unwrap();
    let k2 = run(map.insert(2, 0)).unwrap();

    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            run(map.with(&k1, |v| {
                *v += 1;
                panic!("stop");
            }))
            .unwrap();
        }))
        .is_err()
    );
    assert_eq!(run(map.clone_inner(&k1)).unwrap(), 1);

    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            run(map.with_many([&k1, &k2], |[v1, v2]| {
                *v1 += 1;
                *v2 += 1;
                panic!("stop");
            }))
            .unwrap();
        }))
        .is_err()
    );
    assert_eq!(
        run(map.with_many([&k1, &k2], |[v1, v2]| (*v1, *v2))).unwrap(),
        (2, 1)
    );
}

#[test]
#[cfg(feature = "std")]
fn map_locking() {
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
