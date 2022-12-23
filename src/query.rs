use std::{
    fmt::{self, Debug, Formatter},
    future::Future,
    pin::Pin,
    rc::{Rc, Weak},
};

use crate::{
    cache::{CacheControl, Cacheable},
    config::{error::Error, retry::RetryConfig, CacheTime, NetworkMode, SetOption},
    const_default::ConstDefault,
    futures::future_handle::FutureHandle,
    handle_map::HandleMap,
    listenable::Listenable,
    mutation::MutationOpts,
    status::QueryData,
    weak_link::WeakLink,
};

pub(crate) struct FetchMeta<'link, R, E> {
    pub(crate) data: Listenable<'link, QueryData<R, E>>,
    pub(crate) id: usize,
    pub(crate) future_handles: HandleMap<FutureHandle<'link>>,
    pub(crate) cache_control: CacheControl<'link>,
}

/// A query funnction that can be executed with or without a client
pub struct Query<'link, P, R, E> {
    pub(crate) inner: Rc<QueryInner<'link, P, R, E>>,
}

impl<P, R, E> Debug for Query<'_, P, R, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Query")
            .field("func", &"..")
            .field("hydrate_key", &self.inner.hydrate_key)
            .finish_non_exhaustive()
    }
}

/// Defaults for this query
#[derive(Debug)]
pub struct QueryOpts<'cfg, E: ?Sized> {
    /// See [`CacheTime`]
    pub cache_time: SetOption<CacheTime>,
    /// See [`NetworkMode`]
    pub network_mode: SetOption<NetworkMode>,
    /// See [`RetryConfig`]
    pub retry: SetOption<RetryConfig<'cfg, E>>,
}

impl<'cfg, E: ?Sized> From<MutationOpts<'cfg, E>> for QueryOpts<'cfg, E> {
    fn from(value: MutationOpts<'cfg, E>) -> Self {
        Self {
            cache_time: value.cache_time,
            network_mode: value.network_mode,
            retry: value.retry,
        }
    }
}

impl<E: ?Sized> Clone for QueryOpts<'_, E> {
    fn clone(&self) -> Self {
        Self {
            cache_time: self.cache_time,
            network_mode: self.network_mode,
            retry: self.retry.clone(),
        }
    }
}

impl<E: ?Sized> Default for QueryOpts<'_, E> {
    fn default() -> Self {
        Self::const_default()
    }
}

impl<E: ?Sized> ConstDefault for QueryOpts<'_, E> {
    const DEFAULT: Self = Self::const_default();
}

impl<'cfg, E: ?Sized> QueryOpts<'cfg, E> {
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

    /// Gets the default for [`QueryOpts`] as a const
    #[must_use = "Creating new options has no effect"]
    #[inline]
    pub const fn const_default() -> Self {
        Self {
            cache_time: SetOption::DEFAULT,
            network_mode: SetOption::DEFAULT,
            retry: SetOption::DEFAULT,
        }
    }

    /// Sets [`QueryOpts.cache_time`]
    #[must_use = "Builder pattern"]
    #[inline]
    pub const fn set_cache_time(mut self, cache_time: CacheTime) -> Self {
        self.cache_time = SetOption::set(cache_time);
        self
    }

    /// Sets [`QueryOpts.network_mode`]
    #[must_use = "Builder pattern"]
    #[inline]
    pub const fn set_network_mode(mut self, network_mode: NetworkMode) -> Self {
        self.network_mode = SetOption::set(network_mode);
        self
    }

    /// Sets [`QueryOpts.retry`]
    #[must_use = "Builder pattern"]
    #[inline]
    // Possible drop
    #[allow(clippy::missing_const_for_fn)]
    pub fn set_retry(mut self, retry: RetryConfig<'cfg, E>) -> Self {
        self.retry = SetOption::set(retry);
        self
    }
}

pub(crate) struct QueryInner<'link, P: 'link, R, E> {
    pub(crate) opts: QueryOpts<'link, E>,
    func: Rc<QueryFn<'link, P, R, E>>,
    pub(crate) link: WeakLink<'link, FetchMeta<'link, R, E>>,
    // TODO
    hydrate_key: Option<String>,
}

impl<'link, P, R, E> Cacheable<'link> for Weak<QueryInner<'link, P, R, E>> {
    type LinkData = FetchMeta<'link, R, E>;

    #[inline]
    fn link(&self) -> Option<WeakLink<'link, Self::LinkData>> {
        self.upgrade().map(|q| q.link.clone())
    }
}

pub(crate) type QueryReturn<T, E> = Pin<Box<dyn Future<Output = Result<T, E>>>>;
pub(crate) type NoParam<'func, R, E> = Box<dyn Fn() -> QueryReturn<R, E> + 'func>;
pub(crate) type WithParam<'func, P, R, E> = Box<dyn Fn(&P) -> QueryReturn<R, E> + 'func>;

// TODO
//#[derive(Debug)]
enum QueryFn<'func, P, R, E> {
    NoParam(NoParam<'func, R, E>),
    WithParam(WithParam<'func, P, R, E>),
}

impl<P, R, E> Clone for Query<'_, P, R, E> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<'link, R, E: Error> Query<'link, (), R, E> {
    /// Create new [`Query`] with no arguments
    #[must_use = "No reason to create a Query if you don't use it"]
    #[inline]
    pub fn new(func: impl Fn() -> QueryReturn<R, E> + 'link) -> Self {
        Self::new_inner(QueryFn::NoParam(Box::new(func)), QueryOpts::new())
    }

    /// Create new [`Query`] with no arguments, with configuration options
    #[must_use = "No reason to create a Query if you don't use it"]
    #[inline]
    pub fn new_with_opts(
        func: impl Fn() -> QueryReturn<R, E> + 'link,
        opts: impl Into<QueryOpts<'link, E>>,
    ) -> Self {
        Self::new_inner(QueryFn::NoParam(Box::new(func)), opts.into())
    }

    /// Directly execute query without a client
    ///
    /// # Errors
    /// Will error if the provided query function does
    #[inline]
    pub async fn execute(&self) -> Result<R, E> {
        self.inner.execute_with_arg(&()).await
    }
}

impl<'link, P, R, E: Error> Query<'link, P, R, E> {
    #[inline]
    fn new_inner(func: QueryFn<'link, P, R, E>, opts: QueryOpts<'link, E>) -> Self {
        Self {
            inner: Rc::new(QueryInner {
                opts,
                func: Rc::new(func),
                link: WeakLink::new(),
                hydrate_key: None,
            }),
        }
    }

    #[inline]
    pub(crate) fn new_hydratable(query: &Self, hydratable_key: String) -> Self {
        Self {
            inner: Rc::new(QueryInner {
                opts: query.inner.opts.clone(),
                func: Rc::clone(&query.inner.func),
                link: WeakLink::new(),
                hydrate_key: Some(hydratable_key),
            }),
        }
    }

    /// Create a new [`Query`] with an argument of type ``P``
    #[must_use = "No reason to create a Query if you don't use it"]
    #[inline]
    pub fn new_with_param(func: impl Fn(&P) -> QueryReturn<R, E> + 'link) -> Self {
        Self::new_inner(QueryFn::WithParam(Box::new(func)), QueryOpts::new())
    }

    /// Create a new [`Query`] with an argument of type ``P``, with configuration options
    #[must_use = "No reason to create a Query if you don't use it"]
    #[inline]
    pub fn new_with_param_and_opts(
        func: impl Fn(&P) -> QueryReturn<R, E> + 'link,
        opts: impl Into<QueryOpts<'link, E>>,
    ) -> Self {
        Self::new_inner(QueryFn::WithParam(Box::new(func)), opts.into())
    }

    /// Directly execute query without a client
    ///
    /// # Errors
    /// Will error if the provided query function does
    #[inline]
    pub async fn execute_with_arg(&self, arg: &P) -> Result<R, E> {
        self.inner.execute_with_arg(arg).await
    }
}

impl<P, R, E> QueryInner<'_, P, R, E> {
    #[inline]
    pub(crate) async fn execute_with_arg(&self, arg: &P) -> Result<R, E> {
        match *self.func {
            QueryFn::NoParam(ref func) => func(),
            QueryFn::WithParam(ref func) => func(arg),
        }
        .await
    }
}
