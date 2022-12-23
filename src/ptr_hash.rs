use std::{
    hash::{Hash, Hasher},
    ops::Deref,
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
