use std::{future::Future, pin::Pin, sync::Arc};

use futures::{future, stream::AbortHandle};
use tokio::sync::Mutex;

struct CleanupState<'handle> {
    done: bool,
    cleanup: Vec<Box<dyn Fn() + 'handle>>,
}

pub(crate) struct CleanupHandle<'handle> {
    inner: Arc<Mutex<CleanupState<'handle>>>,
}

pub(crate) struct FutureHandle<'handle> {
    handle: AbortHandle,
    cleanup: Arc<Mutex<CleanupState<'handle>>>,
}

impl<'handle> FutureHandle<'handle> {
    pub(crate) fn cleanup(&self) -> CleanupHandle<'handle> {
        CleanupHandle {
            inner: Arc::clone(&self.cleanup),
        }
    }
}

impl<'handle> CleanupHandle<'handle> {
    pub(crate) async fn add_cleanup(self, f: impl Fn() + 'handle) {
        let mut cleanup = self.inner.lock().await;
        if cleanup.done {
            f();
        } else {
            cleanup.cleanup.push(Box::new(f));
        }
    }
}

impl Drop for FutureHandle<'_> {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

pub(crate) fn spawn_local_handle<'handle>(
    f: impl Future<Output = ()> + 'handle,
) -> FutureHandle<'handle> {
    let cleanup = Arc::new(Mutex::new(CleanupState {
        done: false,
        cleanup: Vec::new(),
    }));
    let boxed: Pin<Box<dyn Future<Output = ()>>> = Box::pin({
        let cleanup = Arc::clone(&cleanup);
        async move {
            f.await;

            let mut cleanup = cleanup.lock().await;
            cleanup.done = true;
            for f in &cleanup.cleanup {
                f();
            }
        }
    });
    let extended: Pin<Box<dyn Future<Output = ()> + 'static>> =
    // SAFETY: We are just transmuting the lifetime here so that we can spawn the future.
    // This is safe because we wrap the future in an `Abortable` future which will be
    // immediately aborted once the reactive scope is dropped.
        unsafe { std::mem::transmute(boxed) };
    let (abortable, handle) = future::abortable(extended);
    super::spawn_local(abortable);
    FutureHandle { handle, cleanup }
}
