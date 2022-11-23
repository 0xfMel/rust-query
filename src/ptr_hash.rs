use std::{
    hash::{Hash, Hasher},
    ops::Deref,
    ptr,
    rc::Weak,
};

pub(crate) struct WeakPtrHash<T: ?Sized>(pub Weak<T>);

impl<T: ?Sized> Hash for WeakPtrHash<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(self.as_ptr() as *const () as usize);
    }
}

impl<T: ?Sized> PartialEq for WeakPtrHash<T> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr_eq(other)
    }
}

impl<T: ?Sized> Eq for WeakPtrHash<T> {}

impl<T: ?Sized> Deref for WeakPtrHash<T> {
    type Target = Weak<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ?Sized> Clone for WeakPtrHash<T> {
    fn clone(&self) -> Self {
        WeakPtrHash(Weak::clone(self))
    }
}

pub(crate) struct BoxPtrHash<T: ?Sized>(pub Box<T>);

impl<T: ?Sized> Hash for BoxPtrHash<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(&*self.0 as *const T as *const () as usize);
    }
}

impl<T: ?Sized> PartialEq for BoxPtrHash<T> {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(&*self.0, &*other.0)
    }
}

impl<T: ?Sized> Eq for BoxPtrHash<T> {}

impl<T: ?Sized> Deref for BoxPtrHash<T> {
    type Target = Box<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
