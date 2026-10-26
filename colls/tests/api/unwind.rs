use super::run;
use colls::*;
use er::ErTest;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn locks() -> ErTest {
    let first = CShared::new(0);
    let second = CShared::new(0);

    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            run(first.with(|_| panic!("stop")));
        }))
        .is_err()
    );
    run(first.with(|_| ()));

    let group = CSharedGroup::new([&first, &second])?;

    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            run(group.with(|_| panic!("stop")));
        }))
        .is_err()
    );
    run(group.with(|_| ()));

    let map = CMap::<u32, u32>::new();
    let k1 = run(map.insert(1, 0))?;
    let k2 = run(map.insert(2, 0))?;

    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            run(map.with(&k1, |_| panic!("stop"))).unwrap();
        }))
        .is_err()
    );
    run(map.with(&k1, |_| ()))?;

    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            run(map.with_many([&k1, &k2], |_| panic!("stop"))).unwrap();
        }))
        .is_err()
    );
    run(map.with_many([&k1, &k2], |_| ()))?;

    Ok(())
}
