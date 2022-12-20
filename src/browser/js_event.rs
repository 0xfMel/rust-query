#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "addEventListener")]
    fn add_event_listener(type_: &str, handler: &Closure<dyn Fn(JsValue)>);
    #[wasm_bindgen(js_name = "removeEventListener")]
    fn remove_event_listener(type_: &str, handler: &Closure<dyn Fn(JsValue)>);
}

/// Handles events on the browser, will listen to event on window when created
/// and will unlisten when dropped
#[derive(Debug)]
pub(crate) struct JsEvent {
    /// The type of event on window to listen to
    type_: String,
    /// The handler closure to call
    handler: Closure<dyn Fn(JsValue)>,
}

impl JsEvent {
    /// Start listening to a new event
    pub(crate) fn new(typ: &str, handler: impl Fn(JsValue) + 'static) -> Self {
        let closure = Closure::<dyn Fn(JsValue)>::new(handler);
        add_event_listener(typ, &closure);

        Self {
            type_: typ.to_owned(),
            handler: closure,
        }
    }
}

impl Drop for JsEvent {
    fn drop(&mut self) {
        remove_event_listener(&self.type_, &self.handler);
    }
}
