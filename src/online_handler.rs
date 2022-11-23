use std::{future::Future, pin::Pin, rc::Rc};

use crate::weak_link::WeakLink;
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use crate::js_event::JsEvent;

#[cfg(target_arch = "wasm32")]
use std::{mem, rc::Weak};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = navigator, js_name = onLine)]
    fn ONLINE() -> bool;
}

#[cfg(target_arch = "wasm32")]
fn is_online() -> bool {
    //ONLINE()
    web_sys::window().unwrap().navigator().on_line()
}

#[cfg(not(target_arch = "wasm32"))]
fn is_online() -> bool {
    true
}

type RetryClosure<'a> = dyn FnOnce() -> Pin<Box<dyn Future<Output = ()> + 'a>> + 'a;

pub(crate) struct OnlineHandler<'a> {
    pub link: Rc<WeakLink<'a, Box<RetryClosure<'a>>>>,
    #[cfg(target_arch = "wasm32")]
    _event: JsEvent,
}

impl OnlineHandler<'_> {
    pub(crate) fn new() -> Self {
        let link = Rc::new(WeakLink::<Box<RetryClosure>>::new());
        Self {
            #[cfg(target_arch = "wasm32")]
            _event: JsEvent::new("online", {
                // lifetime earasure
                // SAFETY: When lifetime Self's lifetime is over, the last strong counter will be dropped and Weak::upgrade will return None
                let link: Weak<WeakLink<Box<RetryClosure>>> =
                    unsafe { mem::transmute(Rc::downgrade(&link)) };
                move |_| {
                    if let Some(link) = link.upgrade() {
                        wasm_bindgen_futures::spawn_local(async move {
                            for retry in link.drain() {
                                retry().await
                            }
                        });
                    }
                }
            }),
            link,
        }
    }

    pub(crate) fn is_online() -> bool {
        is_online()
    }
}
