#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::{
    fmt::{self, Debug, Formatter},
    future::Future,
    pin::Pin,
    rc::{Rc, Weak},
};

use crate::{
    cache::{CacheControl, Cacheable},
    config::{retry::RetryConfig, CacheTime, NetworkMode, SetOption},
    const_default::ConstDefault,
    listenable::Listenable,
    query::QueryOpts,
    status::MutationData,
    weak_link::WeakLink,
};

/// Configuration options for mutations
#[derive(Default, Debug)]
pub struct MutationOpts<'cfg, E: ?Sized> {
    /// See [`CacheTime`]
    pub cache_time: SetOption<CacheTime>,
    /// See [`NetworkMode`]
    pub network_mode: SetOption<NetworkMode>,
    /// See [`RetryConfig`]
    pub retry: SetOption<RetryConfig<'cfg, E>>,
}

impl<E: ?Sized> Clone for MutationOpts<'_, E> {
    fn clone(&self) -> Self {
        Self {
            cache_time: self.cache_time,
            network_mode: self.network_mode,
            retry: self.retry.clone(),
        }
    }
}

impl<E: ?Sized> ConstDefault for MutationOpts<'_, E> {
    const DEFAULT: Self = Self::const_default();
}

impl<'cfg, E: ?Sized> MutationOpts<'cfg, E> {
    /// New options that inherrit all
    #[must_use = "Creating new options has no effect"]
    #[inline]
    pub const fn new() -> Self {
        Self {
            cache_time: SetOption::Inherrit,
            network_mode: SetOption::Inherrit,
            retry: SetOption::Inherrit,
        }
    }

    /// Gets the default for [`MutationOpts`] as a const
    #[must_use = "Creating new options has no effect"]
    #[inline]
    pub const fn const_default() -> Self {
        Self {
            cache_time: SetOption::DEFAULT,
            network_mode: SetOption::DEFAULT,
            retry: SetOption::DEFAULT,
        }
    }

    /// Sets [`MutationOpts.cache_time`]
    #[must_use = "Builder pattern"]
    #[inline]
    pub const fn set_cache_time(mut self, cache_time: CacheTime) -> Self {
        self.cache_time = SetOption::set(cache_time);
        self
    }

    /// Sets [`MutationOpts.network_mode`]
    #[must_use = "Builder pattern"]
    #[inline]
    pub const fn set_network_mode(mut self, network_mode: NetworkMode) -> Self {
        self.network_mode = SetOption::set(network_mode);
        self
    }

    /// Sets [`MutationOpts.retry`]
    #[must_use = "Builder pattern"]
    #[inline]
    // Possible drop
    #[allow(clippy::missing_const_for_fn)]
    pub fn set_retry(mut self, retry: RetryConfig<'cfg, E>) -> Self {
        self.retry = SetOption::set(retry);
        self
    }
}

impl<'cfg, E: ?Sized> From<QueryOpts<'cfg, E>> for MutationOpts<'cfg, E> {
    fn from(value: QueryOpts<'cfg, E>) -> Self {
        Self {
            cache_time: value.cache_time,
            network_mode: value.network_mode,
            retry: value.retry,
        }
    }
}

type CallbackFuture<'cb, T> = Pin<Box<dyn Future<Output = T> + 'cb>>;

type OnSuccess<P, R, C> = dyn for<'cb> Fn(Rc<R>, &'cb P, &'cb Option<C>) -> CallbackFuture<'cb, ()>;
type OnError<P, E, C> = dyn for<'cb> Fn(Rc<E>, &'cb P, &'cb Option<C>) -> CallbackFuture<'cb, ()>;
type OnSettled<P, R, E, C> =
    dyn for<'cb> Fn(Result<Rc<R>, Rc<E>>, &'cb P, &'cb Option<C>) -> CallbackFuture<'cb, ()>;
type OnMutate<P, C> = dyn for<'cb> Fn(&'cb mut P) -> CallbackFuture<'cb, Option<C>>;

/// Callbacks for when a mutation is initiated or has finished
pub struct MutationCallbacks<P, R, E, C> {
    pub(crate) on_success: Option<Box<OnSuccess<P, R, C>>>,
    pub(crate) on_error: Option<Box<OnError<P, E, C>>>,
    pub(crate) on_settled: Option<Box<OnSettled<P, R, E, C>>>,
    pub(crate) on_mutate: Option<Box<OnMutate<P, C>>>,
}

impl<P, R, E, C> Debug for MutationCallbacks<P, R, E, C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("MutationCallbacks")
            .field("on_success", &"..")
            .field("on_error", &"..")
            .field("on_settled", &"..")
            .field("on_mutate", &"..")
            .finish()
    }
}

impl<P, R, E, C> MutationCallbacks<P, R, E, C> {
    /// Container for callbacks for a mutation
    /// Callbacks can be added by chaining method calls
    #[must_use = "Used to construct callbacks for a mutation"]
    #[inline]
    pub fn new() -> Self {
        Self {
            on_success: None,
            on_error: None,
            on_settled: None,
            on_mutate: None,
        }
    }

    /// Add success callback
    /// Will execute when the callback has finished successfully
    #[must_use = "Used to construct callbacks for a mutation"]
    #[inline]
    pub fn on_success<F>(mut self, on_success: F) -> Self
    where
        for<'cb> F: Fn(Rc<R>, &'cb P, &'cb Option<C>) -> CallbackFuture<'cb, ()> + 'cb,
    {
        self.on_success = Some(Box::new(on_success));
        self
    }

    /// Add error callback
    /// Will execute when the callback has finished with an error
    #[must_use = "Used to construct callbacks for a mutation"]
    #[inline]
    pub fn on_error<F>(mut self, on_error: F) -> Self
    where
        for<'cb> F: Fn(Rc<E>, &'cb P, &'cb Option<C>) -> CallbackFuture<'cb, ()> + 'cb,
    {
        self.on_error = Some(Box::new(on_error));
        self
    }

    /// Add settled callback that will be called if the mutation succeeds or not
    #[must_use = "Used to construct callbacks for a mutation"]
    #[inline]
    pub fn on_settled<F>(mut self, on_settled: F) -> Self
    where
        for<'cb> F:
            Fn(Result<Rc<R>, Rc<E>>, &'cb P, &'cb Option<C>) -> CallbackFuture<'cb, ()> + 'cb,
    {
        self.on_settled = Some(Box::new(on_settled));
        self
    }

    /// Add mutate callback that will be called when the mutation begins
    /// Must return a context object that will be passed to the other callbacks: `C`
    #[must_use = "Used to construct callbacks for a mutation"]
    #[inline]
    pub fn on_mutate<F>(mut self, on_mutate: F) -> Self
    where
        for<'cb> F: Fn(&'cb mut P) -> CallbackFuture<'cb, Option<C>> + 'cb,
    {
        self.on_mutate = Some(Box::new(on_mutate));
        self
    }
}

impl<P, R, E, C> Default for MutationCallbacks<P, R, E, C> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct MutateMeta<'link, /*P*/ R, E /*C*/> {
    pub(crate) data: Listenable<'link, MutationData<R, E>>,
    pub(crate) id: usize,
    pub(crate) cache_control: CacheControl<'link>,
}

type MutationReturn<'cb, R, E> = Pin<Box<dyn Future<Output = Result<R, E>> + 'cb>>;
type MutationFn<P, R, E> = dyn for<'cb> Fn(&'cb P) -> MutationReturn<'cb, R, E>;

/// A mutation function that can be executed with or without a client
pub struct Mutation<'link, P, R, E /*C*/> {
    pub(crate) inner: Rc<MutationInner<'link, P, R, E /*C*/>>,
}

pub(crate) struct MutationInner<'link, P: 'link, R, E /*C*/> {
    pub(crate) opts: MutationOpts<'link, E>,
    func: Rc<MutationFn<P, R, E>>,
    pub(crate) link: WeakLink<'link, MutateMeta<'link, /*P*/ R, E /*C*/>>,
    // TODO
    hydration_key: Option<String>,
}

impl<'link, P, R, E> Cacheable<'link> for Weak<MutationInner<'link, P, R, E>> {
    type LinkData = MutateMeta<'link, R, E>;

    #[inline]
    fn link(&self) -> Option<WeakLink<'link, Self::LinkData>> {
        self.upgrade().map(|m| m.link.clone())
    }
}

impl<P, R, E> Debug for Mutation<'_, P, R, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mutation")
            .field("func", &"..")
            .field("hydrate_key", &self.inner.hydration_key)
            .finish_non_exhaustive()
    }
}

impl<'link, P, R, E> Mutation<'link, P, R, E> {
    /// Create a new mutation
    #[must_use = "Creating a mutation as no effect"]
    #[inline]
    pub fn new<F>(func: F) -> Self
    where
        for<'cb> F: Fn(&'cb P) -> MutationReturn<'cb, R, E> + 'cb,
    {
        Self::new_with_opts(func, MutationOpts::new())
    }

    /// Create a new mutation
    #[must_use = "Creating a mutation as no effect"]
    #[inline]
    pub fn new_with_opts<F>(func: F, opts: impl Into<MutationOpts<'link, E>>) -> Self
    where
        for<'cb> F: Fn(&'cb P) -> MutationReturn<'cb, R, E> + 'cb,
    {
        Self {
            inner: Rc::new(MutationInner {
                opts: opts.into(),
                func: Rc::new(func),
                link: WeakLink::new(),
                hydration_key: None,
            }),
        }
    }

    /// Directly execute mutation without a client
    ///
    /// # Errors
    /// Will error if the provided mutation function does
    #[inline]
    pub async fn execute<'cb>(&self, value: &'cb P) -> Result<R, E> {
        self.inner.execute(value).await
    }
}

impl<P, R, E> MutationInner<'_, P, R, E> {
    #[inline]
    pub(crate) async fn execute<'cb>(&self, value: &'cb P) -> Result<R, E> {
        (self.func)(value).await
    }
}
