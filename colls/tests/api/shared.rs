use super::run;
use colls::*;
use er::ErTest;

#[test]
fn value() {
    let value = CShared::new(String::from("cat"));
    let alias = value.clone_ptr();
    let snapshot = run(value.clone_inner());

    run(alias.with(|v| v.push('s')));

    assert_eq!(run(value.clone_inner()), "cats");
    assert_eq!(snapshot, "cat");
}

#[test]
fn group() -> ErTest {
    let first = CShared::new(String::from("first"));
    let second = CShared::new(String::from("second"));
    let third = CShared::new(String::from("third"));
    let alias = first.clone_ptr();

    assert!(CSharedGroup::new([&first, &alias]).is_err());

    let group = CSharedGroup::new([&third, &first, &second])?;

    run(group.with(|[v3, v1, v2]| {
        v3.push_str("-3");
        v1.push_str("-1");
        v2.push_str("-2");
    }));

    assert_eq!(run(first.clone_inner()), "first-1");
    assert_eq!(run(second.clone_inner()), "second-2");
    assert_eq!(run(third.clone_inner()), "third-3");

    Ok(())
}

#[test]
#[cfg(feature = "std")]
fn locking() {
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
}
