//! Universal Cross-Platform Concurrency Abstraction
//!
//! On native Desktop (x86_64, ARM64): Uses Rayon's multi-core work-stealing thread pool (`rayon::prelude::*`).
//! On WebAssembly (`wasm32-unknown-unknown`): Provides transparent zero-overhead sequential fallback traits,
//! allowing identical parallel iteration code syntax (`into_par_iter`, `par_iter`, `par_iter_mut`) to compile
//! and run without COOP/COEP headers or SharedArrayBuffer friction.

#[cfg(not(target_arch = "wasm32"))]
pub use rayon::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
pub use rayon::{join, spawn};

#[cfg(target_arch = "wasm32")]
pub use wasm_fallback::*;

#[cfg(target_arch = "wasm32")]
mod wasm_fallback {
    /// Zero-overhead sequential fallback for `IntoParallelIterator`
    pub trait IntoParallelIterator {
        type Item;
        type Iter: Iterator<Item = Self::Item>;
        fn into_par_iter(self) -> Self::Iter;
    }

    impl<I: IntoIterator> IntoParallelIterator for I {
        type Item = I::Item;
        type Iter = I::IntoIter;
        #[inline]
        fn into_par_iter(self) -> Self::Iter {
            self.into_iter()
        }
    }

    /// Zero-overhead sequential fallback for `IntoParallelRefIterator`
    pub trait IntoParallelRefIterator<'data> {
        type Item;
        type Iter: Iterator<Item = Self::Item>;
        fn par_iter(&'data self) -> Self::Iter;
    }

    impl<'data, T: 'data + ?Sized> IntoParallelRefIterator<'data> for T
    where
        &'data T: IntoIterator,
    {
        type Item = <&'data T as IntoIterator>::Item;
        type Iter = <&'data T as IntoIterator>::IntoIter;
        #[inline]
        fn par_iter(&'data self) -> Self::Iter {
            self.into_iter()
        }
    }

    /// Zero-overhead sequential fallback for `IntoParallelRefMutIterator`
    pub trait IntoParallelRefMutIterator<'data> {
        type Item;
        type Iter: Iterator<Item = Self::Item>;
        fn par_iter_mut(&'data mut self) -> Self::Iter;
    }

    impl<'data, T: 'data + ?Sized> IntoParallelRefMutIterator<'data> for T
    where
        &'data mut T: IntoIterator,
    {
        type Item = <&'data mut T as IntoIterator>::Item;
        type Iter = <&'data mut T as IntoIterator>::IntoIter;
        #[inline]
        fn par_iter_mut(&'data mut self) -> Self::Iter {
            self.into_iter()
        }
    }

    /// Sequential fallback for `rayon::join`
    #[inline]
    pub fn join<A, B, RA, RB>(oper_a: A, oper_b: B) -> (RA, RB)
    where
        A: FnOnce() -> RA,
        B: FnOnce() -> RB,
    {
        (oper_a(), oper_b())
    }

    /// Asynchronous non-blocking task spawn for WebAssembly
    pub fn spawn<F>(func: F)
    where
        F: FnOnce() + 'static,
    {
        wasm_bindgen_futures::spawn_local(async move {
            func();
        });
    }
}

/// Returns the number of logical compute threads currently active.
/// On Desktop: returns Rayon's configured thread count (CPU hardware cores).
/// On WebAssembly: returns 1 (browser cooperative single-thread).
pub fn active_thread_count() -> usize {
    #[cfg(not(target_arch = "wasm32"))]
    {
        rayon::current_num_threads()
    }
    #[cfg(target_arch = "wasm32")]
    {
        1
    }
}

/// Returns true if the current environment supports true hardware multi-threading.
pub fn is_multithreading_supported() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        rayon::current_num_threads() > 1
    }
    #[cfg(target_arch = "wasm32")]
    {
        false
    }
}

