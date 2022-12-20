#[cfg(target_arch = "wasm32")]
pub(crate) use wasm::sleep;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use tokio::time::sleep;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::{future::Future, time::Duration};

    use js_sys::{Function, Promise};
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::JsFuture;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_name = "setTimeout")]
        fn set_timeout(handler: Function, ms: u32) -> i32;
    }

    // For consistent return type between cfgs
    #[allow(clippy::manual_async_fn)]
    pub(crate) fn sleep(duration: Duration) -> impl Future<Output = ()> {
        async move {
            JsFuture::from(Promise::new(&mut |res, _| {
                set_timeout(
                    res,
                    duration
                        .as_millis()
                        .try_into()
                        .expect("duration should not be larger than can fit in a u32"),
                );
            }))
            .await
            .expect("should not fail");
        }
    }
}
