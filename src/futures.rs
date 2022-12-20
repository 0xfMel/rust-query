use std::future::Future;

pub(crate) mod future_handle;

pub(crate) fn spawn_local<T: 'static>(f: impl Future<Output = T> + 'static) {
    #[cfg(not(target_arch = "wasm32"))]
    tokio::task::spawn_local(f);
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(async move {
        drop(f.await);
    });
}
