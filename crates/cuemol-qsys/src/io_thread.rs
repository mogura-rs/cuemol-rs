use cuemol_core::{CueMolError, Result};
use std::fmt;
use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;

struct PipeReader {
    receiver: Receiver<Vec<u8>>,
    current: Vec<u8>,
    offset: usize,
}

impl PipeReader {
    fn new(receiver: Receiver<Vec<u8>>) -> Self {
        Self {
            receiver,
            current: Vec::new(),
            offset: 0,
        }
    }
}

impl Read for PipeReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        loop {
            if self.offset < self.current.len() {
                let available = &self.current[self.offset..];
                let count = available.len().min(output.len());
                output[..count].copy_from_slice(&available[..count]);
                self.offset += count;
                return Ok(count);
            }
            match self.receiver.recv() {
                Ok(data) => {
                    self.current = data;
                    self.offset = 0;
                }
                Err(_) => return Ok(0),
            }
        }
    }
}

/// Worker thread fed incrementally through a blocking byte pipe.
pub struct IOThread<T> {
    sender: Option<Sender<Vec<u8>>>,
    result_receiver: Option<Receiver<Result<T>>>,
    handle: Option<JoinHandle<()>>,
    running: Arc<AtomicBool>,
    supplied_bytes: usize,
}

impl<T> IOThread<T>
where
    T: Send + 'static,
{
    pub fn spawn<F>(loader: F) -> Self
    where
        F: FnOnce(&mut dyn Read) -> Result<T> + Send + 'static,
    {
        let (sender, receiver) = mpsc::channel();
        let (result_sender, result_receiver) = mpsc::channel();
        let running = Arc::new(AtomicBool::new(true));
        let worker_running = Arc::clone(&running);
        let handle = std::thread::spawn(move || {
            let mut reader = PipeReader::new(receiver);
            let result = loader(&mut reader);
            let _ = result_sender.send(result);
            worker_running.store(false, Ordering::SeqCst);
        });
        Self {
            sender: Some(sender),
            result_receiver: Some(result_receiver),
            handle: Some(handle),
            running,
            supplied_bytes: 0,
        }
    }

    pub fn supply_data(&mut self, data: &[u8]) -> bool {
        let Some(sender) = self.sender.as_ref() else {
            return false;
        };
        if sender.send(data.to_vec()).is_err() {
            return false;
        }
        self.supplied_bytes = self.supplied_bytes.saturating_add(data.len());
        true
    }

    pub fn notify_eos(&mut self) -> bool {
        self.sender.take().is_some()
    }

    pub fn supplied_bytes(&self) -> usize {
        self.supplied_bytes
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn wait(mut self) -> Result<T> {
        self.notify_eos();
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .map_err(|_| CueMolError::invalid_argument("I/O worker thread panicked"))?;
        }
        self.running.store(false, Ordering::SeqCst);
        self.result_receiver
            .take()
            .ok_or_else(|| CueMolError::invalid_argument("I/O worker result is unavailable"))?
            .recv()
            .map_err(|_| CueMolError::invalid_argument("I/O worker result channel closed"))?
    }
}

impl<T> fmt::Debug for IOThread<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IOThread")
            .field("running", &self.running.load(Ordering::SeqCst))
            .field("supplied_bytes", &self.supplied_bytes)
            .finish_non_exhaustive()
    }
}

impl<T> Drop for IOThread<T> {
    fn drop(&mut self) {
        self.sender = None;
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        self.running.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_thread_streams_chunks_and_finishes_on_eos() {
        let mut worker = IOThread::spawn(|input| {
            let mut text = String::new();
            input.read_to_string(&mut text)?;
            Ok(text)
        });
        assert!(worker.supply_data(b"alpha"));
        assert!(worker.supply_data(b"-beta"));
        assert_eq!(worker.supplied_bytes(), 10);
        assert!(worker.notify_eos());
        assert!(!worker.supply_data(b"ignored"));
        assert_eq!(worker.wait().unwrap(), "alpha-beta");
    }

    #[test]
    fn io_thread_propagates_loader_errors() {
        let worker = IOThread::<()>::spawn(|_| Err(CueMolError::parse("broken input")));
        assert_eq!(
            worker.wait().unwrap_err(),
            CueMolError::parse("broken input")
        );
    }
}
