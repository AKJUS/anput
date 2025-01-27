use std::{
    error::Error,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Condvar, Mutex,
    },
    thread::{available_parallelism, spawn, JoinHandle},
};

type Job = Box<dyn FnOnce(JobContext) + Send>;

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

    pub fn try_take(&self) -> Option<Vec<T>> {
        todo!()
    }

    pub fn wait(self) -> Option<Vec<T>> {
        self.jobs
            .into_iter()
            .map(|job| job.wait())
            .collect::<Option<Vec<_>>>()
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

struct Worker {
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    #[allow(clippy::type_complexity)]
    fn new(
        index: usize,
        count: usize,
        job_receiver: Arc<Mutex<Receiver<Option<(usize, Job)>>>>,
        notify: Arc<(Mutex<bool>, Condvar)>,
    ) -> Worker {
        let thread = spawn(move || loop {
            let job = {
                let (lock, cvar) = &*notify;
                let Ok(mut running) = lock.lock() else {
                    return;
                };
                while !*running {
                    let Ok(new) = cvar.wait(running) else {
                        return;
                    };
                    running = new;
                }
                let Ok(receiver) = job_receiver.lock() else {
                    return;
                };
                receiver.recv()
            };
            match job {
                Ok(Some((group, task))) => {
                    task(JobContext {
                        worker_thread: index,
                        workers_count: count,
                        work_group: group,
                    });
                }
                _ => break,
            }
        });

        Worker {
            thread: Some(thread),
        }
    }
}

pub struct Jobs {
    workers: Vec<Worker>,
    sender: Sender<Option<(usize, Job)>>,
    notify: Arc<(Mutex<bool>, Condvar)>,
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
        let (sender, receiver) = channel::<Option<(usize, Job)>>();
        let job_receiver = Arc::new(Mutex::new(receiver));
        let notify = Arc::new((Mutex::new(false), Condvar::new()));
        Jobs {
            workers: (0..count)
                .map(|index| {
                    Worker::new(index, count, Arc::clone(&job_receiver), Arc::clone(&notify))
                })
                .collect(),
            sender,
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
        job: impl FnOnce(JobContext) -> T + Send + 'static,
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
        self.sender.send(Some((
            0,
            Box::new(move |ctx| {
                handle2.put(job(ctx));
            }),
        )))?;
        let (lock, cvar) = &*self.notify;
        let mut running = lock.lock().map_err(|error| format!("{}", error))?;
        *running = true;
        cvar.notify_one();
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
                    self.sender.send(Some((
                        group,
                        Box::new(move |ctx| {
                            handle2.put(job(ctx));
                        }),
                    )))?;
                    Ok(handle)
                })
                .collect::<Result<Vec<_>, Box<dyn Error>>>()?,
        };
        let (lock, cvar) = &*self.notify;
        let mut running = lock.lock().map_err(|error| format!("{}", error))?;
        *running = true;
        cvar.notify_one();
        Ok(handle)
    }
}

impl Drop for Jobs {
    fn drop(&mut self) {
        for _ in &self.workers {
            let _ = self.sender.send(None);
        }
        {
            let (lock, cvar) = &*self.notify;
            if let Ok(mut running) = lock.lock() {
                *running = true;
                cvar.notify_all();
            }
        }
        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                let _ = thread.join();
            }
        }
    }
}
