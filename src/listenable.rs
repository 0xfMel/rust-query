use std::{mem, ops::Deref};

use crate::handle_map::{Handle, HandleMap};

type ListenerFn<'func, T> = Box<dyn Fn(T) + 'func>;

pub(crate) struct Listener<'func, T> {
    pub(crate) f: ListenerFn<'func, T>,
    pub(crate) drop_f: Option<Box<dyn FnOnce() + 'func>>,
}

impl<T> Drop for Listener<'_, T> {
    fn drop(&mut self) {
        if let Some(drop_f) = self.drop_f.take() {
            drop_f();
        }
    }
}

pub(crate) struct Listenable<'func, T> {
    value: T,
    listeners: HandleMap<Listener<'func, T>>,
}

impl<'func, T> Listenable<'func, T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            value,
            listeners: HandleMap::new(),
        }
    }

    pub(crate) fn add_listener(&mut self, f: impl Fn(T) + 'func) -> Handle {
        self.listeners.insert(Listener {
            f: Box::new(f),
            drop_f: None,
        })
    }

    pub(crate) fn add_listener_direct(&mut self, listener: Listener<'func, T>) -> Handle {
        self.listeners.insert(listener)
    }

    pub(crate) fn remove_listener(&mut self, handle: Handle) -> usize {
        self.listeners.remove(handle);
        self.listeners.len()
    }

    // Self is drop, can't be consumed by const fn
    #[allow(clippy::missing_const_for_fn)]
    pub(crate) fn unwrap(self) -> T {
        self.value
    }
}

impl<'func, T: Clone + PartialEq> Listenable<'func, T> {
    pub(crate) fn set_cmp(this: &mut Self, value: T) -> Option<T> {
        (this.value != value).then(|| Self::set(this, value))
    }
}

impl<'func, T: Clone> Listenable<'func, T> {
    pub(crate) fn set(this: &mut Self, value: T) -> T {
        let ret = mem::replace(&mut this.value, value);
        Self::notify(this);
        ret
    }

    pub(crate) fn modify<R>(this: &mut Self, func: impl Fn(&mut T) -> R) -> R {
        let ret = func(&mut this.value);
        Self::notify(this);
        ret
    }

    fn notify(this: &Self) {
        for listener in &this.listeners {
            (listener.f)(this.value.clone());
        }
    }
}

impl<T: Default> Default for Listenable<'_, T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> Deref for Listenable<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
