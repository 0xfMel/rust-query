#![cfg(not(target_arch = "wasm32"))]
#![deny(clippy::future_not_send)]

use std::{
    fmt::{self, Debug, Formatter},
    future::Future,
    mem,
    pin::Pin,
    sync::Arc,
    thread,
};

use futures::future;
use tokio::{
    runtime::Builder,
    sync::{mpsc, oneshot, Notify},
    task::{self, LocalSet},
};

use super::QueryClient;

type SsrClientFn<'client> =
    dyn Fn(QueryClient<'client>) -> Pin<Box<dyn Future<Output = ()>>> + 'client + Send + Sync;

#[derive(Debug)]
enum SsrClientReq<'client> {
    With(SsrClientWithReq<'client>),
    Dehydrate(oneshot::Sender<String>),
}

struct SsrClientWithReq<'client> {
    f: Box<SsrClientFn<'client>>,
    notify: Arc<Notify>,
}

impl Debug for SsrClientWithReq<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("SsrClientWithReq")
            .field("f", &"..")
            .field("res", &self.notify)
            .finish()
    }
}

/// Allow usage of [`QueryClient`] in a multi-threaded context, without restricting futures to be unsend
pub struct SsrQueryClient<'client> {
    tx: mpsc::Sender<SsrClientReq<'client>>,
}

impl Debug for SsrQueryClient<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("SsrQueryClient").finish_non_exhaustive()
    }
}

impl<'client> SsrQueryClient<'client> {
    /// Create new [`SsrQueryClient`]
    #[must_use = "No reason to create an SsrQueryClient if you don't use it"]
    pub fn new() -> Self {
        let (tx, mut rx) = mpsc::channel::<SsrClientReq<'client>>(1);
        // SAFETY: LocalSet is aborted when SsrQueryClient is dropped, as tx will be dropped and rx.recv() will return None
        let mut rx: mpsc::Receiver<SsrClientReq<'static>> = unsafe { mem::transmute(rx) };

        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("should be able to create runtime");

        thread::spawn(move || {
            let client = QueryClient::default();
            let local = LocalSet::new();
            let _guard = local.enter();
            let (abortable, handle) = future::abortable(local);

            task::spawn_local(async move {
                while let Some(req) = rx.recv().await {
                    match req {
                        SsrClientReq::With(with) => {
                            task::spawn_local({
                                let client = client.clone();
                                async move {
                                    (with.f)(client).await;
                                    with.notify.notify_one();
                                }
                            });
                        }
                        SsrClientReq::Dehydrate(res) => {
                            // TODO
                            // If caller fails to await `dehydrate` and the future gets dropped, this send will fail
                            // Nothing to handle, just ignore
                            drop(res.send("TODO".to_owned()));
                        }
                    }
                }
                handle.abort();
            });

            // Future abortion should be expected, and not handled
            #[allow(clippy::let_underscore_must_use)]
            let _ = rt.block_on(abortable);
        });

        Self { tx }
    }

    /// Will execute the closure & await the returned future on the [`QueryClient`]'s thread
    /// Takes the [`QueryClient`] as a parameter
    pub async fn with(
        &self,
        f: impl Fn(QueryClient<'client>) -> Pin<Box<dyn Future<Output = ()>>> + 'client + Send + Sync,
    ) {
        let notify = Arc::new(Notify::new());
        self.tx
            .send(SsrClientReq::With(SsrClientWithReq {
                f: Box::new(f),
                notify: Arc::clone(&notify),
            }))
            .await
            .expect("should not be able to fail while `self` is still alive");
        notify.notified().await;
    }

    /// Will get the dehydrated state of the [`QueryClient`]
    pub async fn dehydrate(&self) -> String {
        let (res, rx) = oneshot::channel();
        self.tx
            .send(SsrClientReq::Dehydrate(res))
            .await
            .expect("should not be able to fail while `self` is still alive");
        rx.await
            .expect("should send back a response after completion of passed future")
    }
}

impl Default for SsrQueryClient<'_> {
    fn default() -> Self {
        Self::new()
    }
}
