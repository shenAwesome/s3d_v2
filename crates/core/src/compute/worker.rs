//! Background Task & Worker Dispatcher
//!
//! Provides a unified interface for scheduling background computational tasks
//! without stalling the main UI rendering thread.
//!
//! - On Desktop: Spawns onto Rayon's global threadpool or dedicated OS threads.
//! - On WebAssembly: Dispatches via cooperative non-blocking microtasks or Web Workers.

use std::sync::mpsc::{channel, Receiver, Sender};

/// Dispatches an expensive background task off the UI thread and delivers the result via callback.
pub fn dispatch_background_task<F, R>(task: F, on_complete: impl FnOnce(R) + Send + 'static)
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        rayon::spawn(move || {
            let result = task();
            on_complete(result);
        });
    }

    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(async move {
            let result = task();
            on_complete(result);
        });
    }
}

/// A channel-based background worker pool for streaming asynchronous results back to the main thread.
pub struct BackgroundTaskChannel<T> {
    sender: Sender<T>,
    receiver: Receiver<T>,
}

impl<T: Send + 'static> BackgroundTaskChannel<T> {
    pub fn new() -> Self {
        let (sender, receiver) = channel();
        Self { sender, receiver }
    }

    pub fn sender(&self) -> Sender<T> {
        self.sender.clone()
    }

    /// Spawns a background task that sends its produced value into this channel.
    pub fn spawn<F>(&self, task: F)
    where
        F: FnOnce() -> T + Send + 'static,
    {
        let tx = self.sender.clone();
        #[cfg(not(target_arch = "wasm32"))]
        {
            rayon::spawn(move || {
                let res = task();
                let _ = tx.send(res);
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let res = task();
                let _ = tx.send(res);
            });
        }
    }

    /// Non-blocking drain of completed results on the main thread
    pub fn drain(&self) -> Vec<T> {
        let mut results = Vec::new();
        while let Ok(item) = self.receiver.try_recv() {
            results.push(item);
        }
        results
    }
}

impl<T: Send + 'static> Default for BackgroundTaskChannel<T> {
    fn default() -> Self {
        Self::new()
    }
}

