#![cfg(target_arch = "wasm32")]

use std::{
    future::Future,
    mem,
    pin::Pin,
    rc::{Rc, Weak},
};

use crate::{js_event::JsEvent, weak_link::WeakLink};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = navigator, js_name = onLine)]
    fn ONLINE() -> bool;
}

pub fn is_online() -> bool {
    //ONLINE()
    // prototyping
    #[allow(clippy::unwrap_used)]
    web_sys::window().unwrap().navigator().on_line()
}

type RetryFuture<'fut> = Pin<Box<dyn Future<Output = ()> + 'fut>>;

pub struct OnlineHandler<'link> {
    pub link: Rc<WeakLink<'link, RetryFuture<'link>>>,
    _event: JsEvent,
}

impl OnlineHandler<'_> {
    pub(crate) fn new() -> Self {
        let link = Rc::new(WeakLink::<RetryFuture>::new());
        Self {
            _event: JsEvent::new("online", {
                // lifetime earasure
                let link: Weak<WeakLink<Box<RetryFuture>>> =
                    // SAFETY: When Self's lifetime is over, the last strong counter will be dropped and [`Weak::upgrade`] will return None
                    unsafe { mem::transmute(Rc::downgrade(&link)) };
                move |_| {
                    if let Some(link) = link.upgrade() {
                        wasm_bindgen_futures::spawn_local(async move {
                            for retry in link.drain() {
                                retry.await;
                            }
                        });
                    }
                }
            }),
            link,
        }
    }
}
