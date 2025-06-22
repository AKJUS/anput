use crate::{
    Job, JobContext, JobHandle, JobLocation, JobObject, JobPriority, JobQueue, JobToken, JobsWaker,
    JobsWakerCommand,
};
use intuicio_data::managed::{DynamicManagedLazy, ManagedLazy};
use std::{
    future::poll_fn,
    hash::Hash,
    pin::Pin,
    sync::{Arc, Mutex, RwLock, mpsc::Receiver},
    task::{Context, Poll, Wake},
    thread::{Thread, current, park},
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct OnExit {
    job: Option<JobObject>,
    queue: Arc<JobQueue>,
}

impl Drop for OnExit {
    fn drop(&mut self) {
        if let Some(object) = self.job.take() {
            self.queue.enqueue(object);
        }
    }
}

impl OnExit {
    pub fn invalidate(mut self) {
        self.job = None;
    }
}

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

pub async fn yield_now() {
    wait_polls(1).await
}

pub async fn wait_polls(mut count: usize) {
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
    .await
}

pub async fn wait_time(duration: Duration) -> Duration {
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
    .await
}

pub async fn wait_for_mutex<T>(notify: Arc<Mutex<T>>, mut f: impl FnMut(&T) -> bool) {
    poll_fn(move |cx| {
        if let Ok(value) = notify.try_lock() {
            if f(&value) {
                cx.waker().wake_by_ref();
                Poll::Ready(())
            } else {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        } else {
            cx.waker().wake_by_ref();
            Poll::Ready(())
        }
    })
    .await
}

pub async fn wait_for_rwlock<T>(notify: Arc<RwLock<T>>, mut f: impl FnMut(&T) -> bool) {
    poll_fn(move |cx| {
        if let Ok(notify) = notify.try_read() {
            if f(&notify) {
                cx.waker().wake_by_ref();
                Poll::Ready(())
            } else {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        } else {
            cx.waker().wake_by_ref();
            Poll::Ready(())
        }
    })
    .await
}

pub async fn wait_for_receiver<T>(notify: Receiver<T>) -> T {
    poll_fn(move |cx| {
        if let Ok(value) = notify.try_recv() {
            cx.waker().wake_by_ref();
            Poll::Ready(value)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
    .await
}

pub async fn with_all<T>(
    mut futures: Vec<Pin<Box<dyn Future<Output = T> + Send + Sync>>>,
) -> Vec<T> {
    let mut results = Vec::with_capacity(futures.len());
    poll_fn(move |cx| {
        for future in &mut futures {
            match future.as_mut().poll(cx) {
                Poll::Ready(output) => results.push(output),
                Poll::Pending => {}
            }
        }
        if results.len() == futures.len() {
            cx.waker().wake_by_ref();
            Poll::Ready(std::mem::take(&mut results))
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
    .await
}

pub async fn with_any<T>(
    mut futures: Vec<Pin<Box<dyn Future<Output = T> + Send + Sync>>>,
) -> Option<T> {
    poll_fn(move |cx| {
        for future in &mut futures {
            if let Poll::Ready(output) = future.as_mut().poll(cx) {
                cx.waker().wake_by_ref();
                return Poll::Ready(Some(output));
            }
        }
        cx.waker().wake_by_ref();
        Poll::Pending
    })
    .await
}

pub async fn location() -> JobLocation {
    poll_fn(move |cx| {
        let waker = cx.waker();
        let result = if let Some(waker) = JobsWaker::try_cast(waker) {
            waker.location()
        } else {
            JobLocation::Unknown
        };
        waker.wake_by_ref();
        Poll::Ready(result)
    })
    .await
}

pub async fn context() -> JobContext {
    poll_fn(move |cx| {
        let waker = cx.waker();
        let result = if let Some(waker) = JobsWaker::try_cast(waker) {
            waker.context()
        } else {
            JobContext {
                work_group_index: 0,
                work_groups_count: 1,
            }
        };
        waker.wake_by_ref();
        Poll::Ready(result)
    })
    .await
}

pub async fn priority() -> JobPriority {
    poll_fn(move |cx| {
        let waker = cx.waker();
        let result = if let Some(waker) = JobsWaker::try_cast(waker) {
            waker.priority()
        } else {
            Default::default()
        };
        waker.wake_by_ref();
        Poll::Ready(result)
    })
    .await
}

pub async fn acquire_token<T: Hash>(subject: &T) -> JobToken {
    poll_fn(move |cx| {
        let waker = cx.waker();
        let result = if let Some(waker) = JobsWaker::try_cast(waker) {
            waker.acquire_token(subject)
        } else {
            Some(JobToken::default())
        };
        waker.wake_by_ref();
        match result {
            Some(token) => Poll::Ready(token),
            None => Poll::Pending,
        }
    })
    .await
}

pub async fn acquire_token_timeout<T: Hash>(subject: &T, timeout: Duration) -> JobToken {
    poll_fn(move |cx| {
        let waker = cx.waker();
        let result = if let Some(waker) = JobsWaker::try_cast(waker) {
            waker.acquire_token_timeout(subject, timeout)
        } else {
            Some(JobToken::default())
        };
        waker.wake_by_ref();
        match result {
            Some(token) => Poll::Ready(token),
            None => Poll::Pending,
        }
    })
    .await
}

pub async fn meta<T>(name: &str) -> Option<ManagedLazy<T>> {
    poll_fn(move |cx| {
        let waker = cx.waker();
        let result = if let Some(waker) = JobsWaker::try_cast(waker) {
            waker
                .meta(name)
                .and_then(|lazy| lazy.into_typed::<T>().ok())
        } else {
            None
        };
        waker.wake_by_ref();
        Poll::Ready(result)
    })
    .await
}

pub async fn meta_dynamic(name: &str) -> Option<DynamicManagedLazy> {
    poll_fn(move |cx| {
        let waker = cx.waker();
        let result = if let Some(waker) = JobsWaker::try_cast(waker) {
            waker.meta(name)
        } else {
            None
        };
        waker.wake_by_ref();
        Poll::Ready(result)
    })
    .await
}

pub async fn move_to(location: JobLocation) {
    let mut executed = false;
    poll_fn(move |cx| {
        let waker = cx.waker();
        if executed {
            waker.wake_by_ref();
            Poll::Ready(())
        } else {
            if let Some(waker) = JobsWaker::try_cast(waker) {
                waker.command(JobsWakerCommand::MoveTo(location.clone()));
            }
            executed = true;
            waker.wake_by_ref();
            Poll::Pending
        }
    })
    .await
}

pub async fn spawn_on<F>(location: JobLocation, priority: JobPriority, job: F) -> Option<F::Output>
where
    F: Future + Send + Sync + 'static,
    <F as std::future::Future>::Output: std::marker::Send,
{
    let handle = JobHandle::default();
    let result = handle.clone();
    let mut job = Some(Job(Box::pin(async move {
        handle.put(job.await);
    })));
    poll_fn(move |cx| {
        let waker = cx.waker();
        if let Some(job) = job.take() {
            if let Some(waker) = JobsWaker::try_cast(waker) {
                waker.enqueue(JobObject {
                    job,
                    context: JobContext {
                        work_group_index: 0,
                        work_groups_count: 1,
                    },
                    location: location.clone(),
                    priority,
                    cancel: waker.cancel(),
                });
            }
            waker.wake_by_ref();
            Poll::Pending
        } else {
            waker.wake_by_ref();
            Poll::Ready(())
        }
    })
    .await;
    result.await
}

pub async fn queue_on<T: Send + 'static>(
    location: JobLocation,
    priority: JobPriority,
    job: impl FnOnce(JobContext) -> T + Send + Sync + 'static,
) -> Option<T> {
    let handle = JobHandle::default();
    let result = handle.clone();
    let mut job = Some(Job(Box::pin(async move {
        handle.put(job(context().await));
    })));
    poll_fn(move |cx| {
        let waker = cx.waker();
        if let Some(job) = job.take() {
            if let Some(waker) = JobsWaker::try_cast(waker) {
                waker.enqueue(JobObject {
                    job,
                    context: JobContext {
                        work_group_index: 0,
                        work_groups_count: 1,
                    },
                    location: location.clone(),
                    priority,
                    cancel: waker.cancel(),
                });
            }
            waker.wake_by_ref();
            Poll::Pending
        } else {
            waker.wake_by_ref();
            Poll::Ready(())
        }
    })
    .await;
    result.await
}

pub async fn on_exit(future: impl Future<Output = ()> + Send + Sync + 'static) -> OnExit {
    let mut job = Some(Job(Box::pin(future)));
    poll_fn(move |cx| {
        let waker = cx.waker();
        let result = if let Some(waker) = JobsWaker::try_cast(waker) {
            if let Some(job) = job.take() {
                OnExit {
                    job: Some(JobObject {
                        job,
                        context: JobContext {
                            work_group_index: 0,
                            work_groups_count: 1,
                        },
                        location: JobLocation::current_thread(),
                        priority: JobPriority::High,
                        cancel: waker.cancel(),
                    }),
                    queue: waker.queue(),
                }
            } else {
                Default::default()
            }
        } else {
            Default::default()
        };
        waker.wake_by_ref();
        Poll::Ready(result)
    })
    .await
}
