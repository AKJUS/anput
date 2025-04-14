use std::{
    collections::VecDeque,
    error::Error,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex, RwLock,
    },
    thread::{available_parallelism, spawn, JoinHandle},
    time::Duration,
};

type Job = Box<dyn FnOnce(JobContext) + Send + Sync>;

fn traced_spin_loop() {
    #[cfg(feature = "deadlock-trace")]
    println!(
        "* DEADLOCK BACKTRACE: {}",
        std::backtrace::Backtrace::force_capture()
    );
    std::hint::spin_loop();
}

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
                traced_spin_loop();
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

pub struct AllJobsHandle<T: Send + 'static> {
    jobs: Vec<JobHandle<T>>,
}

impl<T: Send + 'static> Default for AllJobsHandle<T> {
    fn default() -> Self {
        Self {
            jobs: Default::default(),
        }
    }
}

impl<T: Send + 'static> AllJobsHandle<T> {
    pub fn new(value: T) -> Self {
        Self {
            jobs: vec![JobHandle::new(value)],
        }
    }

    pub fn into_inner(self) -> Vec<JobHandle<T>> {
        self.jobs
    }

    pub fn add(&mut self, handle: JobHandle<T>) {
        self.jobs.push(handle);
    }

    pub fn extend(&mut self, handles: impl IntoIterator<Item = JobHandle<T>>) {
        self.jobs.extend(handles);
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

impl<T: Send + 'static> Clone for AllJobsHandle<T> {
    fn clone(&self) -> Self {
        Self {
            jobs: self.jobs.clone(),
        }
    }
}

pub struct AnyJobHandle<T: Send + 'static> {
    jobs: Vec<JobHandle<T>>,
}

impl<T: Send + 'static> Default for AnyJobHandle<T> {
    fn default() -> Self {
        Self {
            jobs: Default::default(),
        }
    }
}

impl<T: Send + 'static> AnyJobHandle<T> {
    pub fn new(value: T) -> Self {
        Self {
            jobs: vec![JobHandle::new(value)],
        }
    }

    pub fn into_inner(self) -> Vec<JobHandle<T>> {
        self.jobs
    }

    pub fn add(&mut self, handle: JobHandle<T>) {
        self.jobs.push(handle);
    }

    pub fn extend(&mut self, handles: impl IntoIterator<Item = JobHandle<T>>) {
        self.jobs.extend(handles);
    }

    pub fn is_done(&self) -> bool {
        self.jobs.iter().any(|job| job.is_done())
    }

    pub fn try_take(&self) -> Option<Option<T>> {
        self.jobs.iter().find_map(|job| job.try_take())
    }

    pub fn wait(self) -> Option<T> {
        loop {
            if let Some(result) = self.try_take() {
                return result;
            } else {
                traced_spin_loop();
            }
        }
    }

    #[cfg(feature = "async")]
    pub async fn into_future(self) -> Option<Option<T>> {
        tokio::task::spawn_blocking(move || self.wait()).await.ok()
    }
}

impl<T: Send + 'static> Clone for AnyJobHandle<T> {
    fn clone(&self) -> Self {
        Self {
            jobs: self.jobs.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobContext {
    pub work_group_index: usize,
    pub work_groups_count: usize,
}

#[derive(Default)]
struct JobQueue {
    /// [(job, work group, work groups count, worker name?)]
    #[allow(clippy::type_complexity)]
    queue: RwLock<VecDeque<(Job, usize, usize, Option<String>)>>,
}

impl JobQueue {
    fn enqueue(
        &self,
        job: Job,
        work_group_index: usize,
        work_groups_count: usize,
        name: Option<String>,
    ) {
        if let Ok(mut queue) = self.queue.write() {
            queue.push_front((job, work_group_index, work_groups_count, name));
        }
    }

    fn dequeue(&self, worker_name: Option<&str>) -> Option<(Job, usize, usize)> {
        let mut queue = self.queue.write().ok()?;
        let (job, group, groups, name) = queue.pop_back()?;
        if name.as_deref() == worker_name {
            Some((job, group, groups))
        } else {
            queue.push_front((job, group, groups, name));
            None
        }
    }
}

struct Worker {
    name: Option<String>,
    thread: Option<JoinHandle<()>>,
    terminate: Arc<AtomicBool>,
}

impl Worker {
    fn new(
        name: Option<String>,
        queue: Arc<JobQueue>,
        notify: Arc<(Mutex<bool>, Condvar)>,
    ) -> Worker {
        let terminate = Arc::new(AtomicBool::default());
        let terminate2 = terminate.clone();
        let name2 = name.clone();
        let thread = spawn(move || loop {
            if terminate2.load(Ordering::Relaxed) {
                return;
            }
            while let Some((job, group, groups)) = queue.dequeue(name2.as_deref()) {
                job(JobContext {
                    work_group_index: group,
                    work_groups_count: groups,
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
                let Ok((new, _)) = cvar.wait_timeout(ready, Duration::from_millis(10)) else {
                    return;
                };
                ready = new;
            }
        });
        Worker {
            name,
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
                .map(|_| Worker::new(None, queue.clone(), notify.clone()))
                .collect(),
            queue,
            notify,
        }
    }

    pub fn with_named_worker(mut self, name: impl ToString) -> Self {
        self.add_named_worker(name);
        self
    }

    pub fn add_named_worker(&mut self, name: impl ToString) {
        self.workers.push(Worker::new(
            Some(name.to_string()),
            self.queue.clone(),
            self.notify.clone(),
        ));
    }

    pub fn remove_named_worker(&mut self, name: &str) {
        if let Some(index) = self.workers.iter().position(|worker| {
            worker
                .name
                .as_deref()
                .map(|n| n == name)
                .unwrap_or_default()
        }) {
            let mut worker = self.workers.swap_remove(index);
            worker.terminate.store(true, Ordering::Relaxed);
            let (lock, cvar) = &*self.notify;
            if let Ok(mut ready) = lock.lock() {
                *ready = true;
            };
            cvar.notify_all();
            if let Some(thread) = worker.thread.take() {
                let _ = thread.join();
            }
        }
    }

    pub fn named_workers(&self) -> impl Iterator<Item = &str> {
        self.workers
            .iter()
            .filter_map(|worker| worker.name.as_deref())
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
        self.queue_inner(None, job)
    }

    pub fn queue_named<T: Send + 'static>(
        &self,
        name: impl ToString,
        job: impl FnOnce(JobContext) -> T + Send + Sync + 'static,
    ) -> Result<JobHandle<T>, Box<dyn Error>> {
        self.queue_inner(Some(name.to_string()), job)
    }

    fn queue_inner<T: Send + 'static>(
        &self,
        name: Option<String>,
        job: impl FnOnce(JobContext) -> T + Send + Sync + 'static,
    ) -> Result<JobHandle<T>, Box<dyn Error>> {
        if self.workers.is_empty() {
            return Ok(JobHandle::new(job(JobContext {
                work_group_index: 0,
                work_groups_count: 1,
            })));
        }
        let handle = JobHandle::<T>::default();
        let handle2 = handle.clone();
        self.queue.enqueue(
            Box::new(move |ctx| {
                handle2.put(job(ctx));
            }),
            0,
            1,
            name,
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
    ) -> Result<AllJobsHandle<T>, Box<dyn Error>> {
        if self.workers.is_empty() {
            return Ok(AllJobsHandle::new(job(JobContext {
                work_group_index: 0,
                work_groups_count: 1,
            })));
        }
        let job = Arc::new(job);
        let handle = AllJobsHandle {
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
                        self.workers.len(),
                        None,
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
    ) -> Result<AllJobsHandle<T>, Box<dyn Error>> {
        if self.workers.is_empty() {
            return Ok(AllJobsHandle {
                jobs: (0..work_groups)
                    .map(|group| {
                        JobHandle::new(job(JobContext {
                            work_group_index: group,
                            work_groups_count: work_groups,
                        }))
                    })
                    .collect(),
            });
        }
        let job = Arc::new(job);
        let handle = AllJobsHandle {
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
                        work_groups,
                        None,
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

pub struct ScopedJobs<'env, T: Send + 'static> {
    jobs: &'env Jobs,
    handles: AllJobsHandle<T>,
}

impl<T: Send + 'static> Drop for ScopedJobs<'_, T> {
    fn drop(&mut self) {
        self.execute_inner();
    }
}

impl<'env, T: Send + 'static> ScopedJobs<'env, T> {
    pub fn new(jobs: &'env Jobs) -> Self {
        Self {
            jobs,
            handles: Default::default(),
        }
    }

    pub fn queue(
        &mut self,
        job: impl FnOnce(JobContext) -> T + Send + Sync + 'env,
    ) -> Result<(), Box<dyn Error>> {
        let job = unsafe {
            std::mem::transmute::<
                Box<dyn FnOnce(JobContext) -> T + Send + Sync + 'env>,
                Box<dyn FnOnce(JobContext) -> T + Send + Sync + 'static>,
            >(Box::new(job))
        };
        self.handles.add(self.jobs.queue(job)?);
        Ok(())
    }

    pub fn queue_named(
        &mut self,
        name: impl ToString,
        job: impl FnOnce(JobContext) -> T + Send + Sync + 'env,
    ) -> Result<(), Box<dyn Error>> {
        let job = unsafe {
            std::mem::transmute::<
                Box<dyn FnOnce(JobContext) -> T + Send + Sync + 'env>,
                Box<dyn FnOnce(JobContext) -> T + Send + Sync + 'static>,
            >(Box::new(job))
        };
        self.handles.add(self.jobs.queue_named(name, job)?);
        Ok(())
    }

    pub fn broadcast(
        &mut self,
        job: impl Fn(JobContext) -> T + Send + Sync + 'env,
    ) -> Result<(), Box<dyn Error>> {
        let job = unsafe {
            std::mem::transmute::<
                Box<dyn Fn(JobContext) -> T + Send + Sync + 'env>,
                Box<dyn Fn(JobContext) -> T + Send + Sync + 'static>,
            >(Box::new(job))
        };
        self.handles.extend(self.jobs.broadcast(job)?.into_inner());
        Ok(())
    }

    pub fn broadcast_n(
        &mut self,
        work_groups: usize,
        job: impl Fn(JobContext) -> T + Send + Sync + 'env,
    ) -> Result<(), Box<dyn Error>> {
        let job = unsafe {
            std::mem::transmute::<
                Box<dyn Fn(JobContext) -> T + Send + Sync + 'env>,
                Box<dyn Fn(JobContext) -> T + Send + Sync + 'static>,
            >(Box::new(job))
        };
        self.handles
            .extend(self.jobs.broadcast_n(work_groups, job)?.into_inner());
        Ok(())
    }

    pub fn execute(mut self) -> Vec<T> {
        self.execute_inner()
    }

    fn execute_inner(&mut self) -> Vec<T> {
        std::mem::take(&mut self.handles).wait().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jobs() {
        let jobs = Jobs::default();
        let data = (0..100).collect::<Vec<_>>();

        let job = jobs
            .queue(move |_| data.iter().copied().sum::<usize>())
            .unwrap();

        let result = job.wait().unwrap();
        assert_eq!(result, 4950);
    }

    #[test]
    fn test_scoped_jobs() {
        let jobs = Jobs::default();
        let mut data = (0..100).collect::<Vec<_>>();

        let mut scope = ScopedJobs::new(&jobs);
        scope
            .queue(|_| {
                for value in &mut data {
                    *value *= 2;
                }
                data.iter().copied().sum::<usize>()
            })
            .unwrap();

        let result = scope.execute().into_iter().sum::<usize>();
        assert_eq!(result, 9900);
    }
}
