#![cfg(target_arch = "wasm32")]

use std::{
    cell::{Cell, RefCell},
    sync::{Arc, Weak},
};

use crate::browser::js_event::JsEvent;
use once_cell::sync::OnceCell;
use tokio::sync::Notify;
use wasm_bindgen::{prelude::*, JsCast};

thread_local! {
    static ONLINE_HANDLER: RefCell<Weak<OnlineHandler>> = RefCell::new(Weak::new());
}

fn get_handler() -> Arc<OnlineHandler> {
    ONLINE_HANDLER.with(|handler| {
        let mut handler = handler.borrow_mut();
        handler.upgrade().unwrap_or_else(|| {
            let this = OnlineHandler::new();
            *handler = Arc::downgrade(&this);
            this
        })
    })
}

// TODO: why no work?
#[wasm_bindgen]
extern "C" {
    type Window;
    type Navigator;

    #[wasm_bindgen(method, getter = navigator)]
    fn navigator(this: &Window) -> Navigator;
    #[wasm_bindgen(method, getter = onLine)]
    fn online(this: &Navigator) -> bool;
}

pub(crate) fn is_online() -> bool {
    let window: Window = js_sys::global()
        .dyn_into()
        .expect("should be able to get Window");
    window.navigator().online()
}

pub(crate) struct OnlineHandler {
    online: Cell<bool>,
    notify: Notify,
    event: OnceCell<JsEvent>,
}

impl OnlineHandler {
    fn check_online(&self) {
        if is_online() {
            self.set_online();
        } else {
            self.online.set(false);
        }
    }

    fn set_online(&self) {
        self.online.set(true);
        self.notify.notify_waiters();
    }

    pub(crate) async fn wait() {
        let this = get_handler();
        while !this.online.get() {
            let notify = this.notify.notified();
            tokio::pin!(notify);
            notify.as_mut().enable();
            this.check_online();
            notify.await;
        }
    }

    fn new() -> Arc<Self> {
        let this = Arc::new(Self {
            online: Cell::new(true),
            notify: Notify::new(),
            event: OnceCell::new(),
        });

        this.event
            .set(JsEvent::new("online", {
                let this = Arc::clone(&this);
                move |_| {
                    this.set_online();
                }
            }))
            .expect("should not fail to set the JsEvent of a newly created Self");
        this
    }
}
