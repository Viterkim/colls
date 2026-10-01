use super::*;
use core::hash::Hash;

#[test]
fn drop_scope() {
    use alloc::rc::Rc;
    use core::{
        cell::Cell,
        future::Future,
        hash::Hasher,
        pin::pin,
        task::{Context, Poll, Waker},
    };

    #[derive(Clone)]
    struct Probe {
        id: u32,
        check: Option<Rc<dyn Fn()>>,
    }
    impl PartialEq for Probe {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl Eq for Probe {}
    impl Hash for Probe {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.id.hash(state);
        }
    }
    impl Drop for Probe {
        fn drop(&mut self) {
            if let Some(check) = &self.check {
                check();
            }
        }
    }

    let map = CMap::<Probe, Probe>::new();
    let alias = map.clone_ptr();
    let drops = Rc::new(Cell::new(0));
    let count = Rc::clone(&drops);
    let probe = Probe {
        id: 0,
        check: Some(Rc::new(move || {
            for shard in &alias.inner.shards {
                #[cfg(feature = "std")]
                assert!(shard.try_lock().is_some(), "drop ran under a shard lock");
                #[cfg(not(feature = "std"))]
                assert!(
                    shard.try_borrow_mut().is_ok(),
                    "drop ran under a shard borrow"
                );
            }
            count.set(count.get() + 1);
        })),
    };

    let mut context = Context::from_waker(Waker::noop());

    let Poll::Ready(Ok(key)) = pin!(map.insert(probe.clone(), probe))
        .as_mut()
        .poll(&mut context)
    else {
        panic!("insert should finish");
    };

    assert!(matches!(
        pin!(map.insert(&key, Probe { id: 0, check: None }))
            .as_mut()
            .poll(&mut context),
        Poll::Ready(Ok(_))
    ));
    assert_eq!(drops.get(), 1);

    assert!(matches!(
        pin!(map.remove(&key)).as_mut().poll(&mut context),
        Poll::Ready(Ok(_))
    ));
    assert_eq!(drops.get(), 2);
}
