use std::{
    collections::VecDeque,
    error::Error,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex, RwLock,
    },
    thread::{available_parallelism, spawn, JoinHandle},
};

type Job = Box<dyn FnOnce(JobContext) + Send + Sync>;

pub struct JobHandle<T: Send + 'static> {
    result: Arc<Mutex<Option<Option<T>>>>,
}

impl<T: Send + 'static> Default for JobHandle<T> {
    fn default() -> Self {
        Self {
            result: Default::default(),
        }
    }
}

impl<T: Send + 'static> JobHandle<T> {
    pub fn new(value: T) -> Self {
        Self {
            result: Arc::new(Mutex::new(Some(Some(value)))),
        }
    }

    pub fn is_done(&self) -> bool {
        self.result
            .try_lock()
            .ok()
            .map(|guard| guard.is_some())
            .unwrap_or_default()
    }

    pub fn try_take(&self) -> Option<Option<T>> {
        self.result
            .try_lock()
            .ok()
            .and_then(|mut result| result.take())
    }

    pub fn wait(self) -> Option<T> {
        loop {
            if let Some(result) = self.try_take() {
                return result;
            } else {
                std::hint::spin_loop();
            }
        }
    }

    #[cfg(feature = "async")]
    pub async fn into_future(self) -> Option<T> {
        tokio::task::spawn_blocking(move || self.wait())
            .await
            .ok()
            .flatten()
    }

    fn put(&self, value: T) {
        if let Ok(mut result) = self.result.lock() {
            *result = Some(Some(value));
        }
    }
}

impl<T: Send + 'static> Clone for JobHandle<T> {
    fn clone(&self) -> Self {
        Self {
            result: Arc::clone(&self.result),
        }
    }
}

pub struct ManyJobsHandle<T: Send + 'static> {
    jobs: Vec<JobHandle<T>>,
}

impl<T: Send + 'static> ManyJobsHandle<T> {
    pub fn new(value: T) -> Self {
        Self {
            jobs: vec![JobHandle::new(value)],
        }
    }

    pub fn is_done(&self) -> bool {
        self.jobs.iter().all(|job| job.is_done())
    }

    pub fn try_take(&self) -> Option<Option<Vec<T>>> {
        self.jobs.iter().map(|job| job.try_take()).collect()
    }

    pub fn wait(self) -> Option<Vec<T>> {
        self.jobs.into_iter().map(|job| job.wait()).collect()
    }

    #[cfg(feature = "async")]
    pub async fn into_future(self) -> Option<Vec<T>> {
        tokio::task::spawn_blocking(move || self.wait())
            .await
            .ok()
            .flatten()
    }
}

impl<T: Send + 'static> Clone for ManyJobsHandle<T> {
    fn clone(&self) -> Self {
        Self {
            jobs: self.jobs.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobContext {
    pub worker_thread: usize,
    pub workers_count: usize,
    pub work_group: usize,
}

#[derive(Default)]
struct JobQueue {
    /// [(job, work group)]
    queue: RwLock<VecDeque<(Job, usize)>>,
}

impl JobQueue {
    fn enqueue(&self, job: Job, group: usize) {
        if let Ok(mut queue) = self.queue.write() {
            queue.push_front((job, group));
        }
    }

    fn dequeue(&self) -> Option<(Job, usize)> {
        self.queue
            .write()
            .ok()
            .and_then(|mut queue| queue.pop_back())
    }
}

struct Worker {
    thread: Option<JoinHandle<()>>,
    terminate: Arc<AtomicBool>,
}

impl Worker {
    fn new(
        index: usize,
        count: usize,
        queue: Arc<JobQueue>,
        notify: Arc<(Mutex<bool>, Condvar)>,
    ) -> Worker {
        let terminate = Arc::new(AtomicBool::default());
        let terminate2 = terminate.clone();
        let thread = spawn(move || loop {
            if terminate2.load(Ordering::Relaxed) {
                return;
            }
            while let Some((job, group)) = queue.dequeue() {
                job(JobContext {
                    worker_thread: index,
                    workers_count: count,
                    work_group: group,
                });
                if terminate2.load(Ordering::Relaxed) {
                    return;
                }
            }
            let (lock, cvar) = &*notify;
            let Ok(mut ready) = lock.lock() else {
                return;
            };
            *ready = false;
            while !*ready {
                let Ok(new) = cvar.wait(ready) else {
                    return;
                };
                ready = new;
            }
        });
        Worker {
            thread: Some(thread),
            terminate,
        }
    }
}

pub struct Jobs {
    workers: Vec<Worker>,
    queue: Arc<JobQueue>,
    /// (ready, cond var)
    notify: Arc<(Mutex<bool>, Condvar)>,
}

impl Drop for Jobs {
    fn drop(&mut self) {
        for worker in &self.workers {
            worker.terminate.store(true, Ordering::Relaxed);
        }
        let (lock, cvar) = &*self.notify;
        if let Ok(mut ready) = lock.lock() {
            *ready = true;
        };
        cvar.notify_all();
        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

impl Default for Jobs {
    fn default() -> Self {
        Self::new(
            available_parallelism()
                .ok()
                .map(|v| v.get())
                .unwrap_or_default(),
        )
    }
}

impl Jobs {
    pub fn new(count: usize) -> Jobs {
        let queue = Arc::new(JobQueue::default());
        let notify = Arc::new((Mutex::default(), Condvar::new()));
        Jobs {
            workers: (0..count)
                .map(|index| Worker::new(index, count, queue.clone(), notify.clone()))
                .collect(),
            queue,
            notify,
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.workers.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.workers.len()
    }

    pub fn queue<T: Send + 'static>(
        &self,
        job: impl FnOnce(JobContext) -> T + Send + Sync + 'static,
    ) -> Result<JobHandle<T>, Box<dyn Error>> {
        if self.workers.is_empty() {
            return Ok(JobHandle::new(job(JobContext {
                worker_thread: 0,
                workers_count: 1,
                work_group: 0,
            })));
        }
        let handle = JobHandle::<T>::default();
        let handle2 = handle.clone();
        self.queue.enqueue(
            Box::new(move |ctx| {
                handle2.put(job(ctx));
            }),
            0,
        );
        let (lock, cvar) = &*self.notify;
        let mut running = lock.lock().map_err(|error| format!("{}", error))?;
        *running = true;
        cvar.notify_all();
        Ok(handle)
    }

    pub fn broadcast<T: Send + 'static>(
        &self,
        job: impl Fn(JobContext) -> T + Send + Sync + 'static,
    ) -> Result<ManyJobsHandle<T>, Box<dyn Error>> {
        if self.workers.is_empty() {
            return Ok(ManyJobsHandle::new(job(JobContext {
                worker_thread: 0,
                workers_count: 1,
                work_group: 0,
            })));
        }
        let job = Arc::new(job);
        let handle = ManyJobsHandle {
            jobs: (0..self.workers.len())
                .map(|group| {
                    let job = Arc::clone(&job);
                    let handle = JobHandle::<T>::default();
                    let handle2 = handle.clone();
                    self.queue.enqueue(
                        Box::new(move |ctx| {
                            handle2.put(job(ctx));
                        }),
                        group,
                    );
                    handle
                })
                .collect::<Vec<_>>(),
        };
        let (lock, cvar) = &*self.notify;
        let mut running = lock.lock().map_err(|error| format!("{}", error))?;
        *running = true;
        cvar.notify_all();
        Ok(handle)
    }

    pub fn broadcast_n<T: Send + 'static>(
        &self,
        work_groups: usize,
        job: impl Fn(JobContext) -> T + Send + Sync + 'static,
    ) -> Result<ManyJobsHandle<T>, Box<dyn Error>> {
        if self.workers.is_empty() {
            return Ok(ManyJobsHandle {
                jobs: (0..work_groups)
                    .map(|group| {
                        JobHandle::new(job(JobContext {
                            worker_thread: 0,
                            workers_count: 1,
                            work_group: group,
                        }))
                    })
                    .collect(),
            });
        }
        let job = Arc::new(job);
        let handle = ManyJobsHandle {
            jobs: (0..work_groups)
                .map(|group| {
                    let job = Arc::clone(&job);
                    let handle = JobHandle::<T>::default();
                    let handle2 = handle.clone();
                    self.queue.enqueue(
                        Box::new(move |ctx| {
                            handle2.put(job(ctx));
                        }),
                        group,
                    );
                    handle
                })
                .collect::<Vec<_>>(),
        };
        let (lock, cvar) = &*self.notify;
        let mut running = lock.lock().map_err(|error| format!("{}", error))?;
        *running = true;
        cvar.notify_all();
        Ok(handle)
    }
}
