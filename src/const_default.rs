/// Allows setting a default for a struct that can be used as a const
pub trait ConstDefault {
    /// The default value
    const DEFAULT: Self;
}

/// Gets the default for [`T`] as a const
#[inline]
#[must_use = "Gets the default, has no effect if unused"]
pub const fn const_default<T: ConstDefault>() -> T {
    T::DEFAULT
}
