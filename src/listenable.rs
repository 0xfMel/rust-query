use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    mem,
    ops::Deref,
    ptr,
};

use crate::ptr_hash::HashBoxPtr;

type ListenerFn<'func, T> = dyn Fn(T) + 'func;

pub(crate) struct Listener<'func, T> {
    pub(crate) f: HashBoxPtr<ListenerFn<'func, T>>,
    pub(crate) drop_f: Option<Box<dyn FnOnce() + 'func>>,
}

impl<T> Drop for Listener<'_, T> {
    fn drop(&mut self) {
        if let Some(drop_f) = self.drop_f.take() {
            drop_f();
        }
    }
}

impl<T> Hash for Listener<'_, T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.f.hash(state);
    }
}

impl<T> PartialEq for Listener<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.f.eq(&other.f)
    }
}

impl<T> Eq for Listener<'_, T> {}

pub(crate) struct Listenable<'func, T> {
    value: T,
    listeners: HashSet<Listener<'func, T>>,
}

pub(crate) struct Handle {
    ptr: *const (),
}

impl<'func, T> Listenable<'func, T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            value,
            listeners: HashSet::new(),
        }
    }

    pub(crate) fn add_listener(&mut self, func: impl Fn(T) + 'func) -> Handle {
        let boxed = Box::new(func);
        let ptr: *const () = ptr::addr_of!(boxed).cast();
        self.listeners.insert(Listener {
            f: HashBoxPtr(boxed),
            drop_f: None,
        });
        Handle { ptr }
    }

    pub(crate) fn add_listener_direct(&mut self, listener: Listener<'func, T>) -> Handle {
        let ptr: *const () = ptr::addr_of!(listener.f.0).cast();
        self.listeners.insert(listener);
        Handle { ptr }
    }

    pub(crate) fn remove_listener(&mut self, handle: &Handle) -> usize {
        self.listeners
            .retain(|e| !ptr::eq(ptr::addr_of!(*e.f.0).cast(), handle.ptr));
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
