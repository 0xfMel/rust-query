use std::{
    hash::{Hash, Hasher},
    ops::Deref,
    ptr,
    rc::Weak,
};

/// Allows the contained [`Weak`] to be hashed using its pointer
pub(crate) struct HashWeakPtr<T: ?Sized>(pub(crate) Weak<T>);

impl<T: ?Sized> Hash for HashWeakPtr<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(self.as_ptr().cast::<()>() as usize);
    }
}

impl<T: ?Sized> PartialEq for HashWeakPtr<T> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr_eq(other)
    }
}

impl<T: ?Sized> Eq for HashWeakPtr<T> {}

impl<T: ?Sized> Deref for HashWeakPtr<T> {
    type Target = Weak<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ?Sized> Clone for HashWeakPtr<T> {
    fn clone(&self) -> Self {
        Self(Weak::clone(self))
    }
}

/// Allows the contained [`Box`] to be hashed using its pointer
pub(crate) struct HashBoxPtr<T: ?Sized>(pub(crate) Box<T>);

impl<T: ?Sized> Hash for HashBoxPtr<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(ptr::addr_of!(*self.0).cast::<()>() as usize);
    }
}

impl<T: ?Sized> PartialEq for HashBoxPtr<T> {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(&*self.0, &*other.0)
    }
}

impl<T: ?Sized> Eq for HashBoxPtr<T> {}

impl<T: ?Sized> Deref for HashBoxPtr<T> {
    type Target = Box<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
