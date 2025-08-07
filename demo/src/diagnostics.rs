use anput::{
    scheduler::GraphSchedulerDiagnosticsEvent, third_party::anput_jobs::JobsDiagnosticsEvent,
};
use std::{
    fs::File,
    io::{Seek, SeekFrom, Write},
    sync::mpsc::Receiver,
    thread::{ThreadId, current},
    time::SystemTime,
};

pub struct ChromeTracing {
    file: File,
    jobs: Receiver<JobsDiagnosticsEvent>,
    scheduler: Receiver<GraphSchedulerDiagnosticsEvent>,
}

impl Drop for ChromeTracing {
    fn drop(&mut self) {
        self.maintain();
        let _ = writeln!(&mut self.file, "{{}}]");
        let _ = self.file.flush();
    }
}

impl ChromeTracing {
    pub fn new(
        mut file: File,
        jobs: Receiver<JobsDiagnosticsEvent>,
        scheduler: Receiver<GraphSchedulerDiagnosticsEvent>,
    ) -> Self {
        let _ = file.seek(SeekFrom::Start(0));
        let _ = writeln!(&mut file, "[");
        Self {
            file,
            jobs,
            scheduler,
        }
    }

    pub fn frame_begin(&mut self) {
        let pid = std::process::id();
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_micros();
        let tid = unsafe { std::mem::transmute::<ThreadId, u64>(current().id()) };
        let _ = writeln!(
            &mut self.file,
            "{{\"name\":\"frame\",\"cat\":\"FRAME\",\"ph\":\"B\",\"ts\":{timestamp},\"pid\":{pid},\"tid\":{tid}}},"
        );
    }

    pub fn frame_end(&mut self) {
        let pid = std::process::id();
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_micros();
        let tid = unsafe { std::mem::transmute::<ThreadId, u64>(current().id()) };
        let _ = writeln!(
            &mut self.file,
            "{{\"name\":\"frame\",\"cat\":\"FRAME\",\"ph\":\"E\",\"ts\":{timestamp},\"pid\":{pid},\"tid\":{tid}}},"
        );
    }

    pub fn maintain(&mut self) {
        let pid = std::process::id();
        while let Ok(event) = self.jobs.try_recv() {
            match event {
                JobsDiagnosticsEvent::JobPollBegin {
                    timestamp,
                    id,
                    location,
                    context,
                    priority,
                    thread_id,
                } => {
                    let timestamp = timestamp
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_micros();
                    let tid = unsafe { std::mem::transmute::<ThreadId, u64>(thread_id) };
                    let _ = writeln!(
                        &mut self.file,
                        "{{\"name\":\"{id}\",\"cat\":\"JOBS\",\"ph\":\"B\",\"ts\":{timestamp},\"pid\":{pid},\"tid\":{tid},\"args\":{{\"location\":\"{location:?}\",\"priority\":\"{priority:?}\",\"context\":\"{context:?}\"}}}},",
                    );
                }
                JobsDiagnosticsEvent::JobPollEnd {
                    timestamp,
                    id,
                    location,
                    context,
                    priority,
                    thread_id,
                    pending,
                    ..
                } => {
                    if !pending {
                        let timestamp = timestamp
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_micros();
                        let tid = unsafe { std::mem::transmute::<ThreadId, u64>(thread_id) };
                        let _ = writeln!(
                            &mut self.file,
                            "{{\"name\":\"{id}\",\"cat\":\"JOBS\",\"ph\":\"E\",\"ts\":{timestamp},\"pid\":{pid},\"tid\":{tid},\"args\":{{\"location\":\"{location:?}\",\"priority\":\"{priority:?}\",\"context\":\"{context:?}\"}}}},",
                        );
                    }
                }
                JobsDiagnosticsEvent::UserEvent {
                    timestamp,
                    id,
                    location,
                    context,
                    priority,
                    thread_id,
                    duration,
                    payload,
                } => {
                    let timestamp = timestamp
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_micros();
                    let tid = unsafe { std::mem::transmute::<ThreadId, u64>(thread_id) };
                    if let Some(duration) = duration {
                        let duration = duration.as_micros();
                        let _ = writeln!(
                            &mut self.file,
                            "{{\"name\":\"{id}\",\"cat\":\"JOBS\",\"ph\":\"X\",\"ts\":{timestamp},\"dur\":{duration},\"pid\":{pid},\"tid\":{tid},\"args\":{{\"location\":\"{location:?}\",\"priority\":\"{priority:?}\",\"context\":\"{context:?}\",\"payload\":\"{payload:?}\"}}}},",
                        );
                    } else {
                        let _ = writeln!(
                            &mut self.file,
                            "{{\"name\":\"{id}\",\"cat\":\"JOBS\",\"ph\":\"I\",\"ts\":{timestamp},\"pid\":{pid},\"tid\":{tid},\"args\":{{\"location\":\"{location:?}\",\"priority\":\"{priority:?}\",\"context\":\"{context:?}\",\"payload\":\"{payload:?}\"}}}},",
                        );
                    }
                }
            };
        }
        while let Ok(event) = self.scheduler.try_recv() {
            match event {
                GraphSchedulerDiagnosticsEvent::RunBegin {
                    timestamp,
                    thread_id,
                } => {
                    let timestamp = timestamp
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_micros();
                    let tid = unsafe { std::mem::transmute::<ThreadId, u64>(thread_id) };
                    let _ = writeln!(
                        &mut self.file,
                        "{{\"name\":\"Run\",\"cat\":\"GRAPH_SCHEDULER\",\"ph\":\"B\",\"ts\":{timestamp},\"pid\":{pid},\"tid\":{tid}}},",
                    );
                }
                GraphSchedulerDiagnosticsEvent::RunEnd {
                    timestamp,
                    thread_id,
                } => {
                    let timestamp = timestamp
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_micros();
                    let tid = unsafe { std::mem::transmute::<ThreadId, u64>(thread_id) };
                    let _ = writeln!(
                        &mut self.file,
                        "{{\"name\":\"Run\",\"cat\":\"GRAPH_SCHEDULER\",\"ph\":\"E\",\"ts\":{timestamp},\"pid\":{pid},\"tid\":{tid}}},",
                    );
                }
                GraphSchedulerDiagnosticsEvent::GroupBegin {
                    timestamp,
                    thread_id,
                    entity,
                    name,
                } => {
                    let timestamp = timestamp
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_micros();
                    let tid = unsafe { std::mem::transmute::<ThreadId, u64>(thread_id) };
                    let name = name.as_deref().unwrap_or("");
                    let _ = writeln!(
                        &mut self.file,
                        "{{\"name\":\"Group:{entity}:{name}\",\"cat\":\"GRAPH_SCHEDULER\",\"ph\":\"B\",\"ts\":{timestamp},\"pid\":{pid},\"tid\":{tid}}},",
                    );
                }
                GraphSchedulerDiagnosticsEvent::GroupEnd {
                    timestamp,
                    thread_id,
                    entity,
                    name,
                } => {
                    let timestamp = timestamp
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_micros();
                    let tid = unsafe { std::mem::transmute::<ThreadId, u64>(thread_id) };
                    let name = name.as_deref().unwrap_or("");
                    let _ = writeln!(
                        &mut self.file,
                        "{{\"name\":\"Group:{entity}:{name}\",\"cat\":\"GRAPH_SCHEDULER\",\"ph\":\"E\",\"ts\":{timestamp},\"pid\":{pid},\"tid\":{tid}}},",
                    );
                }
                GraphSchedulerDiagnosticsEvent::SystemBegin {
                    timestamp,
                    thread_id,
                    entity,
                    name,
                } => {
                    let timestamp = timestamp
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_micros();
                    let tid = unsafe { std::mem::transmute::<ThreadId, u64>(thread_id) };
                    let name = name.as_deref().unwrap_or("");
                    let _ = writeln!(
                        &mut self.file,
                        "{{\"name\":\"System:{entity}:{name}\",\"cat\":\"GRAPH_SCHEDULER\",\"ph\":\"B\",\"ts\":{timestamp},\"pid\":{pid},\"tid\":{tid}}},",
                    );
                }
                GraphSchedulerDiagnosticsEvent::SystemEnd {
                    timestamp,
                    thread_id,
                    entity,
                    name,
                } => {
                    let timestamp = timestamp
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_micros();
                    let tid = unsafe { std::mem::transmute::<ThreadId, u64>(thread_id) };
                    let name = name.as_deref().unwrap_or("");
                    let _ = writeln!(
                        &mut self.file,
                        "{{\"name\":\"System:{entity}:{name}\",\"cat\":\"GRAPH_SCHEDULER\",\"ph\":\"E\",\"ts\":{timestamp},\"pid\":{pid},\"tid\":{tid}}},",
                    );
                }
                GraphSchedulerDiagnosticsEvent::UserBegin {
                    timestamp,
                    thread_id,
                    name,
                    payload,
                } => {
                    let timestamp = timestamp
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_micros();
                    let tid = unsafe { std::mem::transmute::<ThreadId, u64>(thread_id) };
                    let _ = writeln!(
                        &mut self.file,
                        "{{\"name\":\"User:{name}\",\"cat\":\"GRAPH_SCHEDULER\",\"ph\":\"B\",\"ts\":{timestamp},\"pid\":{pid},\"tid\":{tid},\"args\":{{\"payload\":\"{payload:?}\"}}}},",
                    );
                }
                GraphSchedulerDiagnosticsEvent::UserEnd {
                    timestamp,
                    thread_id,
                    name,
                    payload,
                } => {
                    let timestamp = timestamp
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_micros();
                    let tid = unsafe { std::mem::transmute::<ThreadId, u64>(thread_id) };
                    let _ = writeln!(
                        &mut self.file,
                        "{{\"name\":\"User:{name}\",\"cat\":\"GRAPH_SCHEDULER\",\"ph\":\"E\",\"ts\":{timestamp},\"pid\":{pid},\"tid\":{tid},\"args\":{{\"payload\":\"{payload:?}\"}}}},",
                    );
                }
                GraphSchedulerDiagnosticsEvent::UserInstant {
                    timestamp,
                    thread_id,
                    name,
                    payload,
                } => {
                    let timestamp = timestamp
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_micros();
                    let tid = unsafe { std::mem::transmute::<ThreadId, u64>(thread_id) };
                    let _ = writeln!(
                        &mut self.file,
                        "{{\"name\":\"User:{name}\",\"cat\":\"GRAPH_SCHEDULER\",\"ph\":\"I\",\"ts\":{timestamp},\"pid\":{pid},\"tid\":{tid},\"args\":{{\"payload\":\"{payload:?}\"}}}},",
                    );
                }
            }
        }
    }
}
