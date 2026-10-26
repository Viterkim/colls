use super::*;
use alloc::rc::Rc;
use core::{
    future::Future,
    hash::{Hash, Hasher},
    pin::pin,
    task::{Context, Poll, Waker},
};

#[test]
fn drop_scope() {
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
        })),
    };

    let operations = async {
        let key = map.insert(probe.clone(), probe).await?;

        map.insert(&key, Probe { id: 0, check: None }).await?;
        map.remove(&key).await?;

        Ok::<_, CMapError>(())
    };

    let mut context = Context::from_waker(Waker::noop());

    let Poll::Ready(Ok(())) = pin!(operations).as_mut().poll(&mut context) else {
        panic!("map operations should finish");
    };
}
