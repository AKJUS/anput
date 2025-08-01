use std::{
    collections::VecDeque,
    marker::PhantomData,
    sync::{
        Arc, Mutex, Weak,
        mpsc::{Receiver, Sender},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventHandle<T: Clone + Send> {
    id: usize,
    _phantom: PhantomData<fn() -> T>,
}

pub struct EventDispatcher<T: Clone + Send> {
    senders: Vec<(usize, Sender<T>)>,
    sinks: Vec<(usize, Weak<Mutex<VecDeque<T>>>)>,
    id_generator: usize,
}

impl<T: Clone + Send> Default for EventDispatcher<T> {
    fn default() -> Self {
        EventDispatcher {
            senders: Default::default(),
            sinks: Default::default(),
            id_generator: 0,
        }
    }
}

impl<T: Clone + Send> EventDispatcher<T> {
    pub fn bind_sender(&mut self, sender: Sender<T>) -> EventHandle<T> {
        let id = self.id_generator;
        self.id_generator = self.id_generator.wrapping_add(1);
        self.senders.push((id, sender));
        EventHandle {
            id,
            _phantom: PhantomData,
        }
    }

    pub fn bind_sender_make(&mut self) -> (EventHandle<T>, Receiver<T>) {
        let (sender, receiver) = std::sync::mpsc::channel();
        let handle = self.bind_sender(sender);
        (handle, receiver)
    }

    pub fn bind_sink(&mut self, sink: &EventSink<T>) -> EventHandle<T> {
        let id = self.id_generator;
        self.id_generator = self.id_generator.wrapping_add(1);
        self.sinks.push((id, Arc::downgrade(&sink.queue)));
        EventHandle {
            id,
            _phantom: PhantomData,
        }
    }

    pub fn bind_sink_make(&mut self) -> (EventHandle<T>, EventSink<T>) {
        let sink = EventSink {
            queue: Arc::new(Mutex::new(VecDeque::new())),
        };
        let handle = self.bind_sink(&sink);
        (handle, sink)
    }

    pub fn unbind(&mut self, handle: EventHandle<T>) {
        self.senders.retain(|(id, _)| *id != handle.id);
        self.sinks.retain(|(id, _)| *id != handle.id);
    }

    pub fn unbind_all(&mut self) {
        self.senders.clear();
        self.sinks.clear();
    }

    pub fn dispatch(&self, event: &T) {
        for (_, sender) in &self.senders {
            let _ = sender.send(event.clone());
        }
        for (_, queue) in &self.sinks {
            if let Some(queue) = queue.upgrade() {
                if let Ok(mut queue) = queue.lock() {
                    queue.push_back(event.clone());
                }
            }
        }
    }

    pub fn dispatch_to_alive(&mut self, event: &T) {
        self.senders
            .retain(|(_, sender)| sender.send(event.clone()).is_ok());
        self.sinks.retain(|(_, queue)| {
            if let Some(queue) = queue.upgrade() {
                if let Ok(mut queue) = queue.lock() {
                    queue.push_back(event.clone());
                }
                true
            } else {
                false
            }
        });
    }
}

#[derive(Debug)]
pub struct EventSink<T> {
    queue: Arc<Mutex<VecDeque<T>>>,
}

impl<T> EventSink<T> {
    pub fn len(&self) -> usize {
        self.queue.lock().map_or(0, |queue| queue.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        if let Ok(mut queue) = self.queue.lock() {
            queue.clear();
        }
    }

    pub fn recv(&self) -> Option<T> {
        self.queue.lock().ok()?.pop_front()
    }

    pub fn try_recv(&self) -> Option<T> {
        self.queue.try_lock().ok()?.pop_front()
    }

    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        std::iter::from_fn(|| self.recv())
    }

    pub fn try_iter(&self) -> impl Iterator<Item = T> + '_ {
        std::iter::from_fn(|| self.try_recv())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event() {
        let mut event = EventDispatcher::<String>::default();
        let (handle, receiver) = event.bind_sender_make();

        event.dispatch(&"Hello".to_string());
        assert_eq!(receiver.recv().unwrap(), "Hello");

        event.unbind(handle);
        event.dispatch(&"World".to_string());
        assert!(receiver.try_recv().is_err());
    }
}
