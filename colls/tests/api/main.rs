mod map;
mod shared;
mod unwind;

use std::{
    future::Future,
    pin::pin,
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
