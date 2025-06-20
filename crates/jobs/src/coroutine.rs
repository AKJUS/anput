use crate::{JobLocation, JobsWaker, JobsWakerCommand};
use std::{
    future::poll_fn,
    sync::Arc,
    task::{Context, Poll, Wake},
    thread::{Thread, current, park},
    time::{Duration, Instant},
};

pub fn block_on<F: Future>(future: F) -> F::Output {
    struct ThreadWaker(Thread);

    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    let mut future = Box::pin(future);
    let t = current();
    let waker = Arc::new(ThreadWaker(t)).into();
    let mut ctx = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut ctx) {
            Poll::Ready(output) => return output,
            Poll::Pending => park(),
        }
    }
}

pub fn yield_now() -> impl Future<Output = ()> {
    wait_polls(1)
}

pub fn wait_polls(mut count: usize) -> impl Future<Output = ()> {
    poll_fn(move |cx| {
        if count == 0 {
            cx.waker().wake_by_ref();
            Poll::Ready(())
        } else {
            count -= 1;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
}

pub fn wait_time(duration: Duration) -> impl Future<Output = Duration> {
    let timer = Instant::now();
    poll_fn(move |cx| {
        let elapsed = timer.elapsed();
        if elapsed >= duration {
            cx.waker().wake_by_ref();
            Poll::Ready(elapsed - duration)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
}

pub fn location() -> impl Future<Output = JobLocation> {
    let mut executed = false;
    poll_fn(move |cx| {
        let waker = cx.waker();
        if executed {
            let location = if let Some(waker) = JobsWaker::try_cast(waker) {
                waker.location()
            } else {
                JobLocation::Unknown
            };
            waker.wake_by_ref();
            Poll::Ready(location)
        } else {
            executed = true;
            waker.wake_by_ref();
            Poll::Pending
        }
    })
}

pub fn move_to_local() -> impl Future<Output = ()> {
    let mut executed = false;
    poll_fn(move |cx| {
        let waker = cx.waker();
        if executed {
            waker.wake_by_ref();
            Poll::Ready(())
        } else {
            if let Some(waker) = JobsWaker::try_cast(waker) {
                waker.command(JobsWakerCommand::MoveToLocal);
            }
            executed = true;
            waker.wake_by_ref();
            Poll::Pending
        }
    })
}

pub fn move_to_unnamed_worker() -> impl Future<Output = ()> {
    let mut executed = false;
    poll_fn(move |cx| {
        let waker = cx.waker();
        if executed {
            waker.wake_by_ref();
            Poll::Ready(())
        } else {
            if let Some(waker) = JobsWaker::try_cast(waker) {
                waker.command(JobsWakerCommand::MoveToUnnamedWorker);
            }
            executed = true;
            waker.wake_by_ref();
            Poll::Pending
        }
    })
}

pub fn move_to_named_worker(name: impl ToString) -> impl Future<Output = ()> {
    let name = name.to_string();
    let mut executed = false;
    poll_fn(move |cx| {
        let waker = cx.waker();
        if executed {
            waker.wake_by_ref();
            Poll::Ready(())
        } else {
            if let Some(waker) = JobsWaker::try_cast(waker) {
                waker.command(JobsWakerCommand::MoveToNamedWorker(name.clone()));
            }
            executed = true;
            waker.wake_by_ref();
            Poll::Pending
        }
    })
}
