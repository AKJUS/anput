pub mod coroutine;

use intuicio_data::managed::{DynamicManagedLazy, ManagedLazy};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    error::Error,
    hash::{DefaultHasher, Hash, Hasher},
    pin::Pin,
    sync::{
        Arc, Condvar, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender},
    },
    task::{Context, Poll, RawWaker, RawWakerVTable, Wake, Waker},
    thread::{JoinHandle, ThreadId, available_parallelism, spawn},
    time::{Duration, Instant},
};

enum Job {
    Closure(Box<dyn FnOnce(JobContext) + Send + Sync>),
    Future(Pin<Box<dyn Future<Output = ()> + Send + Sync>>),
}

impl Job {
    fn poll(self, cx: &mut Context<'_>, jcx: JobContext) -> Option<Self> {
        match self {
            Job::Closure(job) => {
                job(jcx);
                None
            }
            Job::Future(mut future) => match future.as_mut().poll(cx) {
                Poll::Ready(_) => None,
                Poll::Pending => Some(Job::Future(future)),
            },
        }
    }
}

#[inline]
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

impl<T: Send + 'static> Future for JobHandle<T> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(result) = self.try_take() {
            cx.waker().wake_by_ref();
            Poll::Ready(result)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
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

    pub fn many(handles: impl IntoIterator<Item = JobHandle<T>>) -> Self {
        Self {
            jobs: handles.into_iter().collect(),
        }
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
        self.is_done()
            .then(|| self.jobs.iter().flat_map(|job| job.try_take()).collect())
    }

    pub fn wait(self) -> Option<Vec<T>> {
        self.jobs.into_iter().map(|job| job.wait()).collect()
    }
}

impl<T: Send + 'static> Clone for AllJobsHandle<T> {
    fn clone(&self) -> Self {
        Self {
            jobs: self.jobs.clone(),
        }
    }
}

impl<T: Send + 'static> Future for AllJobsHandle<T> {
    type Output = Option<Vec<T>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(result) = self.try_take() {
            cx.waker().wake_by_ref();
            Poll::Ready(result)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
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

    pub fn many(handles: impl IntoIterator<Item = JobHandle<T>>) -> Self {
        Self {
            jobs: handles.into_iter().collect(),
        }
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
        self.is_done()
            .then(|| self.jobs.iter().find_map(|job| job.try_take()).flatten())
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
}

impl<T: Send + 'static> Clone for AnyJobHandle<T> {
    fn clone(&self) -> Self {
        Self {
            jobs: self.jobs.clone(),
        }
    }
}

impl<T: Send + 'static> Future for AnyJobHandle<T> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(result) = self.try_take() {
            cx.waker().wake_by_ref();
            Poll::Ready(result)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
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
    /// [(job, work group, work groups count, worker name?, thread id?)]
    #[allow(clippy::type_complexity)]
    queue: RwLock<VecDeque<(Job, usize, usize, Option<String>, Option<ThreadId>)>>,
}

impl JobQueue {
    fn is_empty(&self) -> bool {
        self.queue.read().map_or(true, |queue| queue.is_empty())
    }

    fn enqueue(
        &self,
        job: Job,
        work_group_index: usize,
        work_groups_count: usize,
        name: Option<String>,
        thread_id: Option<ThreadId>,
    ) {
        if let Ok(mut queue) = self.queue.write() {
            queue.push_front((job, work_group_index, work_groups_count, name, thread_id));
        }
    }

    fn enqueue_important(
        &self,
        job: Job,
        work_group_index: usize,
        work_groups_count: usize,
        name: Option<String>,
        thread_id: Option<ThreadId>,
    ) {
        if let Ok(mut queue) = self.queue.write() {
            queue.push_back((job, work_group_index, work_groups_count, name, thread_id));
        }
    }

    fn dequeue(&self, worker_name: Option<&str>) -> Option<(Job, usize, usize)> {
        let mut queue = self.queue.write().ok()?;
        let (job, group, groups, name, thread_id) = queue.pop_back()?;
        if thread_id == Some(std::thread::current().id()) || name.as_deref() == worker_name {
            Some((job, group, groups))
        } else {
            queue.push_front((job, group, groups, name, thread_id));
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
        local_queue: Arc<JobQueue>,
        meta: Arc<RwLock<HashMap<String, DynamicManagedLazy>>>,
        hash_tokens: Arc<Mutex<HashSet<u64>>>,
        notify: Arc<(Mutex<bool>, Condvar)>,
    ) -> Worker {
        let terminate = Arc::new(AtomicBool::default());
        let terminate2 = terminate.clone();
        let name2 = name.clone();
        let thread = spawn(move || {
            let mut temp = vec![];
            let (waker, receiver) = JobsWaker::new_waker(
                match name2 {
                    Some(ref name) => JobLocation::NamedWorker(name.clone()),
                    None => JobLocation::UnnamedWorker,
                },
                meta,
                hash_tokens,
            );
            let mut cx = Context::from_waker(&waker);
            loop {
                if terminate2.load(Ordering::Relaxed) {
                    return;
                }
                while let Some((job, group, groups)) = queue.dequeue(name2.as_deref()) {
                    let mut notify_workers = false;
                    if let Some(job) = job.poll(
                        &mut cx,
                        JobContext {
                            work_group_index: group,
                            work_groups_count: groups,
                        },
                    ) {
                        let mut move_to = JobMoveTo::None;
                        for command in receiver.try_iter() {
                            notify_workers = true;
                            match command {
                                JobsWakerCommand::MoveTo(location) => match location {
                                    JobLocation::Local => {
                                        move_to = JobMoveTo::Local;
                                    }
                                    JobLocation::UnnamedWorker => {
                                        move_to = JobMoveTo::AnyWorker;
                                    }
                                    JobLocation::NamedWorker(name) => {
                                        move_to = JobMoveTo::NamedWorker(name);
                                    }
                                    JobLocation::Thread(thread) => {
                                        move_to = JobMoveTo::Thread(thread);
                                    }
                                    JobLocation::Unknown => {}
                                },
                                JobsWakerCommand::ScheduleOn(location, job) => match location {
                                    JobLocation::Local => {
                                        local_queue.enqueue_important(job, 0, 1, None, None);
                                    }
                                    JobLocation::UnnamedWorker => {
                                        queue.enqueue_important(job, 0, 1, None, None);
                                    }
                                    JobLocation::NamedWorker(name) => {
                                        queue.enqueue_important(job, 0, 1, Some(name), None);
                                    }
                                    JobLocation::Thread(thread) => {
                                        queue.enqueue_important(job, 0, 1, None, Some(thread));
                                    }
                                    JobLocation::Unknown => {}
                                },
                            }
                        }
                        match move_to {
                            JobMoveTo::None => {
                                temp.push((job, group, groups, name2.clone(), None));
                            }
                            JobMoveTo::Local => {
                                local_queue.enqueue(job, group, groups, None, None);
                            }
                            JobMoveTo::AnyWorker => {
                                temp.push((job, group, groups, None, None));
                            }
                            JobMoveTo::NamedWorker(name) => {
                                temp.push((job, group, groups, Some(name), None));
                            }
                            JobMoveTo::Thread(thread) => {
                                temp.push((job, group, groups, None, Some(thread)));
                            }
                        }
                    }
                    if terminate2.load(Ordering::Relaxed) {
                        return;
                    }
                    if notify_workers {
                        let (lock, cvar) = &*notify;
                        if let Ok(mut running) = lock.lock() {
                            *running = true;
                        }
                        cvar.notify_all();
                    }
                }
                for (job, group, groups, name, thread) in temp.drain(..) {
                    queue.enqueue(job, group, groups, name, thread);
                }
                if !queue.is_empty() {
                    continue;
                }
                let (lock, cvar) = &*notify;
                let Ok(mut ready) = lock.lock() else {
                    return;
                };
                loop {
                    let Ok((new, _)) = cvar.wait_timeout(ready, Duration::from_millis(10)) else {
                        return;
                    };
                    ready = new;
                    if *ready {
                        break;
                    }
                }
            }
        });
        Worker {
            name,
            thread: Some(thread),
            terminate,
        }
    }
}

pub(crate) enum JobsWakerCommand {
    MoveTo(JobLocation),
    ScheduleOn(JobLocation, Job),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum JobMoveTo {
    None,
    Local,
    AnyWorker,
    NamedWorker(String),
    Thread(ThreadId),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum JobLocation {
    #[default]
    Unknown,
    Local,
    UnnamedWorker,
    NamedWorker(String),
    Thread(ThreadId),
}

impl JobLocation {
    pub fn named_worker(name: impl ToString) -> Self {
        JobLocation::NamedWorker(name.to_string())
    }

    pub fn thread(thread: ThreadId) -> Self {
        JobLocation::Thread(thread)
    }

    pub fn current_thread() -> Self {
        JobLocation::Thread(std::thread::current().id())
    }
}

#[derive(Default)]
pub struct JobToken {
    hash_tokens: Arc<Mutex<HashSet<u64>>>,
    hash: u64,
}

impl Drop for JobToken {
    fn drop(&mut self) {
        let mut hash_tokens = self.hash_tokens.lock().unwrap();
        hash_tokens.remove(&self.hash);
    }
}

pub(crate) struct JobsWaker {
    sender: Sender<JobsWakerCommand>,
    location: JobLocation,
    meta: Arc<RwLock<HashMap<String, DynamicManagedLazy>>>,
    hash_tokens: Arc<Mutex<HashSet<u64>>>,
}

impl JobsWaker {
    const VTABLE: RawWakerVTable =
        RawWakerVTable::new(Self::vtable_clone, |_| {}, |_| {}, Self::vtable_drop);

    fn vtable_clone(data: *const ()) -> RawWaker {
        let arc = unsafe { Arc::<Self>::from_raw(data as *const Self) };
        let cloned = arc.clone();
        std::mem::forget(arc);
        RawWaker::new(Arc::into_raw(cloned) as *const (), &Self::VTABLE)
    }

    fn vtable_drop(data: *const ()) {
        let _ = unsafe { Arc::from_raw(data as *const Self) };
    }

    pub fn new_waker(
        location: JobLocation,
        meta: Arc<RwLock<HashMap<String, DynamicManagedLazy>>>,
        hash_tokens: Arc<Mutex<HashSet<u64>>>,
    ) -> (Waker, Receiver<JobsWakerCommand>) {
        let (sender, receiver) = std::sync::mpsc::channel();
        let arc = Arc::new(Self {
            sender,
            location,
            meta,
            hash_tokens,
        });
        let raw = RawWaker::new(Arc::into_raw(arc) as *const (), &Self::VTABLE);
        (unsafe { Waker::from_raw(raw) }, receiver)
    }

    pub fn try_cast(waker: &Waker) -> Option<&Self> {
        if waker.vtable() == &Self::VTABLE {
            unsafe { waker.data().cast::<Self>().as_ref() }
        } else {
            None
        }
    }

    pub fn command(&self, command: JobsWakerCommand) {
        let _ = self.sender.send(command);
    }

    pub fn location(&self) -> JobLocation {
        self.location.clone()
    }

    pub fn meta(&self, name: &str) -> Option<DynamicManagedLazy> {
        let meta = self.meta.read().ok()?;
        meta.get(name).cloned()
    }

    pub fn acquire_token<T: Hash>(&self, subject: &T) -> Option<JobToken> {
        let mut hasher = DefaultHasher::new();
        subject.hash(&mut hasher);
        let hash = hasher.finish();
        let mut hash_tokens = self.hash_tokens.lock().unwrap();
        if hash_tokens.contains(&hash) {
            None
        } else {
            hash_tokens.insert(hash);
            Some(JobToken {
                hash_tokens: self.hash_tokens.clone(),
                hash,
            })
        }
    }

    pub fn acquire_token_timeout<T: Hash>(
        &self,
        subject: &T,
        timeout: Duration,
    ) -> Option<JobToken> {
        let mut hasher = DefaultHasher::new();
        subject.hash(&mut hasher);
        let hash = hasher.finish();
        let timer = Instant::now();
        while timer.elapsed() < timeout {
            let mut hash_tokens = self.hash_tokens.try_lock().unwrap();
            if hash_tokens.contains(&hash) {
                traced_spin_loop();
                continue;
            } else {
                hash_tokens.insert(hash);
                return Some(JobToken {
                    hash_tokens: self.hash_tokens.clone(),
                    hash,
                });
            }
        }
        None
    }
}

impl Wake for JobsWaker {
    fn wake(self: Arc<Self>) {}
}

pub struct Jobs {
    workers: Vec<Worker>,
    queue: Arc<JobQueue>,
    local_queue: Arc<JobQueue>,
    meta: Arc<RwLock<HashMap<String, DynamicManagedLazy>>>,
    hash_tokens: Arc<Mutex<HashSet<u64>>>,
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
        let local_queue = Arc::new(JobQueue::default());
        let notify = Arc::new((Mutex::default(), Condvar::new()));
        let meta = Arc::new(RwLock::new(HashMap::default()));
        let hash_tokens = Arc::new(Mutex::new(HashSet::default()));
        Jobs {
            workers: (0..count)
                .map(|_| {
                    Worker::new(
                        None,
                        queue.clone(),
                        local_queue.clone(),
                        meta.clone(),
                        hash_tokens.clone(),
                        notify.clone(),
                    )
                })
                .collect(),
            queue,
            local_queue,
            meta,
            hash_tokens,
            notify,
        }
    }

    pub fn with_unnamed_worker(mut self) -> Self {
        self.add_unnamed_worker();
        self
    }

    pub fn with_named_worker(mut self, name: impl ToString) -> Self {
        self.add_named_worker(name);
        self
    }

    pub fn add_unnamed_worker(&mut self) {
        self.workers.push(Worker::new(
            None,
            self.queue.clone(),
            self.local_queue.clone(),
            self.meta.clone(),
            self.hash_tokens.clone(),
            self.notify.clone(),
        ));
    }

    pub fn add_named_worker(&mut self, name: impl ToString) {
        self.workers.push(Worker::new(
            Some(name.to_string()),
            self.queue.clone(),
            self.local_queue.clone(),
            self.meta.clone(),
            self.hash_tokens.clone(),
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

    pub fn unnamed_workers(&self) -> usize {
        self.workers
            .iter()
            .filter(|worker| worker.name.is_none())
            .count()
    }

    pub fn named_workers(&self) -> impl Iterator<Item = &str> {
        self.workers
            .iter()
            .filter_map(|worker| worker.name.as_deref())
    }

    pub fn set_meta(&self, name: impl ToString, value: DynamicManagedLazy) {
        let mut meta = self.meta.write().unwrap();
        meta.insert(name.to_string(), value);
    }

    pub fn get_meta<T>(&self, name: &str) -> Option<ManagedLazy<T>> {
        let meta = self.meta.read().unwrap();
        meta.get(name)
            .cloned()
            .and_then(|value| value.into_typed::<T>().ok())
    }

    pub fn get_meta_dynamic(&self, name: &str) -> Option<DynamicManagedLazy> {
        let meta = self.meta.read().unwrap();
        meta.get(name).cloned()
    }

    pub fn run_local(&self) {
        self.run_local_timeout(Duration::MAX);
    }

    pub fn run_local_timeout(&self, timeout: Duration) {
        let mut temp = vec![];
        let (waker, receiver) = JobsWaker::new_waker(
            JobLocation::Local,
            self.meta.clone(),
            self.hash_tokens.clone(),
        );
        let mut cx = Context::from_waker(&waker);
        let timer = Instant::now();
        while let Some((job, group, groups)) = self.local_queue.dequeue(None) {
            let mut notify_workers = false;
            if let Some(job) = job.poll(
                &mut cx,
                JobContext {
                    work_group_index: group,
                    work_groups_count: groups,
                },
            ) {
                let mut move_to = JobMoveTo::None;
                for command in receiver.try_iter() {
                    notify_workers = true;
                    match command {
                        JobsWakerCommand::MoveTo(location) => match location {
                            JobLocation::Local => {
                                move_to = JobMoveTo::Local;
                            }
                            JobLocation::UnnamedWorker => {
                                move_to = JobMoveTo::AnyWorker;
                            }
                            JobLocation::NamedWorker(name) => {
                                move_to = JobMoveTo::NamedWorker(name);
                            }
                            JobLocation::Thread(thread) => {
                                move_to = JobMoveTo::Thread(thread);
                            }
                            JobLocation::Unknown => {}
                        },
                        JobsWakerCommand::ScheduleOn(location, job) => match location {
                            JobLocation::Local => {
                                self.local_queue.enqueue_important(job, 0, 1, None, None);
                            }
                            JobLocation::UnnamedWorker => {
                                self.queue.enqueue_important(job, 0, 1, None, None);
                            }
                            JobLocation::NamedWorker(name) => {
                                self.queue.enqueue_important(job, 0, 1, Some(name), None);
                            }
                            JobLocation::Thread(thread) => {
                                self.queue.enqueue_important(job, 0, 1, None, Some(thread));
                            }
                            JobLocation::Unknown => {}
                        },
                    }
                }
                match move_to {
                    JobMoveTo::None | JobMoveTo::Local => {
                        temp.push((job, group, groups));
                    }
                    JobMoveTo::AnyWorker => {
                        self.queue.enqueue(job, group, groups, None, None);
                    }
                    JobMoveTo::NamedWorker(name) => {
                        self.queue.enqueue(job, group, groups, Some(name), None);
                    }
                    JobMoveTo::Thread(thread) => {
                        self.queue.enqueue(job, group, groups, None, Some(thread));
                    }
                }
            }
            if notify_workers {
                let (lock, cvar) = &*self.notify;
                if let Ok(mut running) = lock.lock() {
                    *running = true;
                }
                cvar.notify_all();
            }
            if timer.elapsed() >= timeout {
                break;
            }
        }
        for (job, group, groups) in temp {
            self.local_queue.enqueue(job, group, groups, None, None);
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

    pub fn spawn_on<T: Send + 'static>(
        &self,
        location: JobLocation,
        job: impl Future<Output = T> + Send + Sync + 'static,
    ) -> Result<JobHandle<T>, Box<dyn Error>> {
        let handle = JobHandle::<T>::default();
        let handle2 = handle.clone();
        let job = Job::Future(Box::pin(async move {
            handle2.put(job.await);
        }));
        let (local, name, thread_id) = match location {
            JobLocation::Local => (true, None, None),
            JobLocation::UnnamedWorker => (false, None, None),
            JobLocation::NamedWorker(name) => (false, Some(name), None),
            JobLocation::Thread(thread) => (false, None, Some(thread)),
            JobLocation::Unknown => {
                return Err("Unknown job location".into());
            }
        };
        self.schedule(local, name, thread_id, handle, job)
    }

    pub fn queue_on<T: Send + 'static>(
        &self,
        location: JobLocation,
        job: impl FnOnce(JobContext) -> T + Send + Sync + 'static,
    ) -> Result<JobHandle<T>, Box<dyn Error>> {
        let handle = JobHandle::<T>::default();
        let handle2 = handle.clone();
        let job = Job::Closure(Box::new(move |ctx| {
            handle2.put(job(ctx));
        }));
        let (local, name, thread_id) = match location {
            JobLocation::Local => (true, None, None),
            JobLocation::UnnamedWorker => (false, None, None),
            JobLocation::NamedWorker(name) => (false, Some(name), None),
            JobLocation::Thread(thread) => (false, None, Some(thread)),
            JobLocation::Unknown => {
                return Err("Unknown job location".into());
            }
        };
        self.schedule(local, name, thread_id, handle, job)
    }

    fn schedule<T: Send + 'static>(
        &self,
        local: bool,
        name: Option<String>,
        thread_id: Option<ThreadId>,
        handle: JobHandle<T>,
        job: Job,
    ) -> Result<JobHandle<T>, Box<dyn Error>> {
        if local || self.workers.is_empty() {
            self.local_queue.enqueue(job, 0, 1, name, None);
        } else {
            self.queue.enqueue(job, 0, 1, name, thread_id);
        }
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
        self.broadcast_n(self.workers.len(), job)
    }

    pub fn broadcast_n<T: Send + 'static>(
        &self,
        work_groups: usize,
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
            jobs: (0..work_groups)
                .map(|group| {
                    let job = Arc::clone(&job);
                    let handle = JobHandle::<T>::default();
                    let handle2 = handle.clone();
                    self.queue.enqueue_important(
                        Job::Closure(Box::new(move |ctx| {
                            handle2.put(job(ctx));
                        })),
                        group,
                        work_groups,
                        None,
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

    pub fn spawn_on(
        &mut self,
        location: JobLocation,
        job: impl Future<Output = T> + Send + Sync + 'env,
    ) -> Result<(), Box<dyn Error>> {
        let job = unsafe {
            std::mem::transmute::<
                Pin<Box<dyn Future<Output = T> + Send + Sync + 'env>>,
                Pin<Box<dyn Future<Output = T> + Send + Sync + 'static>>,
            >(Box::pin(job))
        };
        let handle = self.jobs.spawn_on(location, job)?;
        self.handles.add(handle);
        Ok(())
    }

    pub fn queue_on(
        &mut self,
        location: JobLocation,
        job: impl FnOnce(JobContext) -> T + Send + Sync + 'env,
    ) -> Result<(), Box<dyn Error>> {
        let job = unsafe {
            std::mem::transmute::<
                Box<dyn FnOnce(JobContext) -> T + Send + Sync + 'env>,
                Box<dyn FnOnce(JobContext) -> T + Send + Sync + 'static>,
            >(Box::new(job))
        };
        self.handles.add(self.jobs.queue_on(location, job)?);
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
    use crate::coroutine::{
        acquire_token, block_on, location, meta, move_to, queue_on, spawn_on, with_all, with_any,
        yield_now,
    };
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn test_jobs() {
        let jobs = Jobs::default();
        let data = (0..100).collect::<Vec<_>>();
        let data2 = data.clone();

        let job = jobs
            .queue_on(JobLocation::UnnamedWorker, move |_| {
                data.into_iter().sum::<usize>()
            })
            .unwrap();

        let result = job.wait().unwrap();
        assert_eq!(result, 4950);

        let job = jobs
            .queue_on(JobLocation::Local, move |_| {
                data2.into_iter().sum::<usize>()
            })
            .unwrap();

        while !job.is_done() {
            jobs.run_local();
        }
        let result = job.try_take().unwrap().unwrap();
        assert_eq!(result, 4950);

        let job = jobs.broadcast(move |ctx| ctx.work_group_index).unwrap();
        let result = job.wait().unwrap().into_iter().sum::<usize>();
        assert_eq!(result, {
            let mut accum = 0;
            for index in 0..jobs.workers.len() {
                accum += index;
            }
            accum
        });

        let job = jobs
            .broadcast_n(10, move |ctx| ctx.work_group_index)
            .unwrap();
        let result = job.wait().unwrap().into_iter().sum::<usize>();
        assert_eq!(result, {
            let mut accum = 0;
            for index in 0..10 {
                accum += index;
            }
            accum
        });
    }

    #[test]
    fn test_scoped_jobs() {
        let jobs = Jobs::default();
        let mut data = (0..100).collect::<Vec<_>>();

        let mut scope = ScopedJobs::new(&jobs);
        scope
            .queue_on(JobLocation::UnnamedWorker, |_| {
                for value in &mut data {
                    *value *= 2;
                }
                data.iter().copied().sum::<usize>()
            })
            .unwrap();

        let result = scope.execute().into_iter().sum::<usize>();
        assert_eq!(result, 9900);
    }

    #[test]
    fn test_futures() {
        let jobs = Jobs::default();
        let data = (0..100).collect::<Vec<_>>();
        let data2 = data.clone();

        let job = jobs
            .spawn_on(JobLocation::UnnamedWorker, async move {
                let mut result = 0;
                for value in data {
                    result += value;
                    yield_now().await;
                }
                result
            })
            .unwrap();

        let result = block_on(job).unwrap();
        assert_eq!(result, 4950);

        let job = jobs
            .spawn_on(JobLocation::Local, async move {
                let mut result = 0;
                for value in data2 {
                    result += value;
                    yield_now().await;
                }
                result
            })
            .unwrap();

        while !job.is_done() {
            jobs.run_local();
        }
        let result = job.try_take().unwrap().unwrap();
        assert_eq!(result, 4950);

        let job = jobs
            .spawn_on(JobLocation::UnnamedWorker, async {
                let result = Arc::new(AtomicUsize::new(0));
                let result1 = result.clone();
                let result2 = result.clone();
                with_all(vec![
                    Box::pin(async move {
                        result1.fetch_add(1, Ordering::SeqCst);
                    }),
                    Box::pin(async move {
                        result2.fetch_add(2, Ordering::SeqCst);
                    }),
                ])
                .await;
                result.load(Ordering::SeqCst)
            })
            .unwrap();
        let result = block_on(job).unwrap();
        assert_eq!(result, 3);

        let job = jobs
            .spawn_on(JobLocation::UnnamedWorker, async {
                let result = Arc::new(AtomicUsize::new(0));
                let result1 = result.clone();
                let result2 = result.clone();
                with_any(vec![
                    Box::pin(async move {
                        result1.store(1, Ordering::SeqCst);
                    }),
                    Box::pin(async move {
                        result2.store(2, Ordering::SeqCst);
                    }),
                ])
                .await;
                result.load(Ordering::SeqCst)
            })
            .unwrap();
        let result = block_on(job).unwrap();
        assert!(result > 0);
    }

    #[test]
    fn test_futures_move() {
        let jobs = Jobs::new(1).with_named_worker("foo");

        let job = jobs
            .spawn_on(JobLocation::Local, async {
                yield_now().await;
                // A: Local
                println!("A: {:?}", location().await);
                move_to(JobLocation::UnnamedWorker).await;
                // B: UnnamedWorker
                println!("B: {:?}", location().await);
                move_to(JobLocation::named_worker("foo")).await;
                // C: NamedWorker("foo")
                println!("C: {:?}", location().await);
                move_to(JobLocation::Local).await;
                // D: Local
                println!("D: {:?}", location().await);
                42
            })
            .unwrap();

        while !job.is_done() {
            jobs.run_local();
        }
        let result = job.try_take().unwrap().unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn test_futures_schedule() {
        let jobs = Jobs::new(1).with_named_worker("foo");

        let job = jobs
            .spawn_on(JobLocation::Local, async {
                spawn_on(JobLocation::Local, async {
                    println!("A: {:?}", location().await);
                })
                .await;
                spawn_on(JobLocation::UnnamedWorker, async {
                    println!("B: {:?}", location().await);
                })
                .await;
                spawn_on(JobLocation::named_worker("foo"), async {
                    println!("C: {:?}", location().await);
                })
                .await;
                queue_on(JobLocation::Local, |_| {
                    println!("D: Local closure");
                })
                .await;
                queue_on(JobLocation::UnnamedWorker, |_| {
                    println!("E: Unnamed worker closure");
                })
                .await;
                queue_on(JobLocation::named_worker("foo"), |_| {
                    println!("F: Named worker closure");
                })
                .await;
                42
            })
            .unwrap();

        while !job.is_done() {
            jobs.run_local();
        }
        let result = job.try_take().unwrap().unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn test_futures_meta() {
        let jobs = Jobs::default();
        let mut value = 42usize;
        let (value_lazy, _value_lifetime) = DynamicManagedLazy::make(&mut value);
        jobs.set_meta("value", value_lazy);

        let job = jobs
            .spawn_on(JobLocation::UnnamedWorker, async {
                let value = meta::<usize>("value").await.unwrap();
                *value.read().unwrap()
            })
            .unwrap();

        let result = block_on(job).unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn test_futures_acquire_token() {
        let jobs = Jobs::new(3);

        let a = jobs
            .spawn_on(JobLocation::UnnamedWorker, async {
                let _token = acquire_token(&"foo").await;
                std::thread::sleep(Duration::from_millis(50));
                for i in 0..10 {
                    println!("{}", i);
                    std::thread::sleep(Duration::from_millis(10));
                }
            })
            .unwrap();
        let b = jobs
            .spawn_on(JobLocation::UnnamedWorker, async {
                let _token = acquire_token(&"foo").await;
                std::thread::sleep(Duration::from_millis(50));
                for i in 10..20 {
                    println!("{}", i);
                    std::thread::sleep(Duration::from_millis(10));
                }
            })
            .unwrap();
        block_on(AllJobsHandle::many([a, b])).unwrap();

        #[cfg(not(miri))]
        {
            use std::path::Path;

            async fn load_file(path: impl AsRef<Path>) -> Option<String> {
                let path = path.as_ref();
                let _token = acquire_token(&path).await;
                std::fs::read_to_string(path).ok()
            }

            async fn save_file(path: impl AsRef<Path>, content: &str) {
                let path = path.as_ref();
                let _token = acquire_token(&path).await;
                let _ = std::fs::write(path, content);
            }

            const PATH: &str = "./resources/test.txt";
            let content = "Hello, Jobs!".repeat(1000);

            let a = jobs
                .spawn_on(JobLocation::UnnamedWorker, async move {
                    std::thread::sleep(Duration::from_millis(50));
                    save_file(PATH, &content).await;
                })
                .unwrap();
            let b = jobs
                .spawn_on(JobLocation::UnnamedWorker, async {
                    std::thread::sleep(Duration::from_millis(50));
                    let _ = load_file(PATH).await;
                })
                .unwrap();
            block_on(AllJobsHandle::many([a, b])).unwrap();
        }
    }
}
