#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;
use web_sys::Event;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "addEventListener")]
    fn ADD_EVENT_LISTENER(type_: &str, handler: &Closure<dyn Fn(Event)>);
    #[wasm_bindgen(js_name = "removeEventListener")]
    fn REMOVE_EVENT_LISTENER(type_: &str, handler: &Closure<dyn Fn(Event)>);
}

pub(crate) struct JsEvent {
    type_: String,
    handler: Closure<dyn Fn(Event)>,
}

impl JsEvent {
    pub(crate) fn new(type_: &str, handler: impl Fn(Event) + 'static) -> Self {
        let closure = Closure::<dyn Fn(Event)>::new(handler);
        ADD_EVENT_LISTENER(type_, &closure);

        Self {
            type_: type_.to_string(),
            handler: closure,
        }
    }
}

impl Drop for JsEvent {
    fn drop(&mut self) {
        REMOVE_EVENT_LISTENER(&self.type_, &self.handler);
    }
}
