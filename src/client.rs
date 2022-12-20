#![cfg_attr(
    not(target_arch = "wasm32"),
    allow(dead_code, unused_variables, unused_mut, clippy::unused_async)
)]

use std::{
    cell::RefCell,
    collections::HashSet,
    fmt::{self, Debug, Formatter},
    ptr,
    rc::Rc,
    time::Duration,
};

use tokio::sync::Notify;

use crate::{
    atomic_id,
    cache::{mutation::MutationCache, query::QueryCache, CacheControl},
    config::{
        error::Error,
        resolve::{self, ConfigOption},
        retry::RetryConfig,
        CacheTime, NetworkMode, SetOption,
    },
    const_default::ConstDefault,
    futures::future_handle,
    listenable::Listenable,
    mutation::{MutateMeta, Mutation, MutationCallbacks, MutationOpts},
    ptr_hash::HashBoxPtr,
    query::{FetchMeta, Query, QueryOpts},
    sleep,
    status::{
        FetchResult, FetchResultWaited, LoadingStatus, MutateError, MutationData, NoConnection,
        NoConnectionInner, QueryData, QueryStatus,
    },
    weak_link::Entry,
};

/// Engine-side only client objects
#[cfg(not(target_arch = "wasm32"))]
pub mod engine;

/// Configuration options for this client
#[derive(Debug, Default, Clone)]
pub struct ClientOpts<'cfg> {
    /// See [`CacheTime`]
    pub cache_time: SetOption<CacheTime>,
    /// See [`NetworkMode`]
    pub network_mode: SetOption<NetworkMode>,
    /// See [`RetryConfig`]
    pub retry: SetOption<RetryConfig<'cfg, dyn Error + 'cfg>>,
    /// Default options for queries executed on this client
    pub query: Option<QueryOpts<'cfg, dyn Error + 'cfg>>,
    /// Default options for mutations executed on this client
    pub mutation: Option<MutationOpts<'cfg, dyn Error + 'cfg>>,
}

impl<'cfg> From<QueryOpts<'cfg, dyn Error + 'cfg>> for ClientOpts<'cfg> {
    fn from(value: QueryOpts<'cfg, dyn Error + 'cfg>) -> Self {
        Self {
            cache_time: value.cache_time,
            network_mode: value.network_mode,
            retry: value.retry,
            ..Default::default()
        }
    }
}

impl<'cfg> From<MutationOpts<'cfg, dyn Error + 'cfg>> for ClientOpts<'cfg> {
    fn from(value: MutationOpts<'cfg, dyn Error + 'cfg>) -> Self {
        Self {
            cache_time: value.cache_time,
            network_mode: value.network_mode,
            retry: value.retry,
            ..Default::default()
        }
    }
}

impl ConstDefault for ClientOpts<'_> {
    const DEFAULT: Self = Self::const_default();
}

impl<'cfg> ClientOpts<'cfg> {
    /// New options that inherrit all
    #[must_use = "Creating new options has no effect"]
    #[inline]
    pub const fn new() -> Self {
        Self {
            cache_time: SetOption::Inherrit,
            network_mode: SetOption::Inherrit,
            retry: SetOption::Inherrit,
            query: None,
            mutation: None,
        }
    }

    /// Gets the default for [`ClientOpts`] as a const
    #[must_use = "Creating new options has no effect"]
    #[inline]
    pub const fn const_default() -> Self {
        Self {
            cache_time: SetOption::DEFAULT,
            network_mode: SetOption::DEFAULT,
            retry: SetOption::DEFAULT,
            query: None,
            mutation: None,
        }
    }

    /// Sets [`ClientOpts.cache_time`]
    #[must_use = "Builder pattern"]
    #[inline]
    pub const fn set_cache_time(mut self, cache_time: CacheTime) -> Self {
        self.cache_time = SetOption::set(cache_time);
        self
    }

    /// Sets [`ClientOpts.network_mode`]
    #[must_use = "Builder pattern"]
    #[inline]
    pub const fn set_network_mode(mut self, network_mode: NetworkMode) -> Self {
        self.network_mode = SetOption::set(network_mode);
        self
    }

    /// Sets [`ClientOpts.retry`]
    #[must_use = "Builder pattern"]
    #[inline]
    pub fn set_retry(mut self, retry: RetryConfig<'cfg, dyn Error + 'cfg>) -> Self {
        self.retry = SetOption::set(retry);
        self
    }

    /// Sets [`ClientOpts.query`]
    #[must_use = "Builder pattern"]
    #[inline]
    pub fn set_query(mut self, query: impl Into<QueryOpts<'cfg, dyn Error + 'cfg>>) -> Self {
        self.query = Some(query.into());
        self
    }

    /// Sets [`ClientOpts.mutation`]
    #[must_use = "Builder pattern"]
    #[inline]
    pub fn set_mutation(
        mut self,
        mutation: impl Into<MutationOpts<'cfg, dyn Error + 'cfg>>,
    ) -> Self {
        self.mutation = Some(mutation.into());
        self
    }
}

/// A client that can be configured and used to execute queries and mutations, and cache their results
pub struct QueryClient<'link> {
    inner: Rc<QueryClientInner<'link>>,
}

impl Debug for QueryClient<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueryClient")
            .field("opts", &self.inner.opts)
            .field("query_cache", &self.inner.query_cache)
            .finish()
    }
}

struct QueryClientInner<'link> {
    opts: ClientOpts<'link>,
    pub(crate) query_cache: Rc<QueryCache<'link>>,
    pub(crate) mutation_cache: Rc<MutationCache<'link>>,
}

impl Default for QueryClient<'_> {
    #[inline]
    fn default() -> Self {
        Self::new(ClientOpts::new())
    }
}

impl Clone for QueryClient<'_> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<'link> QueryClient<'link> {
    /// Create a new [`QueryClient`] with provided options
    #[inline]
    #[must_use = "Only used to create `QueryClient`, no effect if not used"]
    pub fn new(opts: impl Into<ClientOpts<'link>>) -> Self {
        Self::new_with_caches(
            opts,
            Rc::new(QueryCache::default()),
            Rc::new(MutationCache::default()),
        )
    }

    /// Create a [`QueryClient`] with provided options, and attached to a given [`QueryCache`]
    #[inline]
    #[must_use = "Only used to create `QueryClient`, no effect if not used"]
    pub fn new_with_caches(
        opts: impl Into<ClientOpts<'link>>,
        query_cache: Rc<QueryCache<'link>>,
        mutation_cache: Rc<MutationCache<'link>>,
    ) -> Self {
        Self {
            inner: Rc::new(QueryClientInner {
                opts: opts.into(),
                query_cache,
                mutation_cache,
            }),
        }
    }

    /// Get [`QueryCache`] this client is attached to
    #[inline]
    #[must_use = "Only gets `QueryCache`, not effect if not used"]
    pub fn query_cache(&self) -> &Rc<QueryCache<'link>> {
        &self.inner.query_cache
    }

    #[inline]
    pub(crate) async fn fetch_with_arg<P, R, E: Error>(
        &self,
        query: &Query<'link, P, R, E>,
        arg: P,
    ) -> FetchResult<R, E> {
        let id = query.inner.link.with_or_else(
            &self.inner.query_cache.link_target,
            || self.new_fetch_meta(query),
            |e| e.id,
        );

        return Rc::clone(&self.inner)
            .fetch_with_arg_inner(Rc::clone(&query.inner), arg, id, 1)
            .await;
    }

    /// Execute mutation on this [`QueryClient`]
    ///
    /// # Errors
    /// Will error if the mutation function errors
    ///
    /// # Panics
    /// Will always panic on engine-side
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn mutate<P, R, E, C>(
        &self,
        _mutation: &Mutation<'link, P, R, E>,
        _value: P,
        _default_cb: Option<&MutationCallbacks<P, R, E, C>>,
        _cb: Option<MutationCallbacks<P, R, E, C>>,
    ) -> Result<Rc<R>, MutateError<E>> {
        panic!("Should not mutate on the engine");
    }

    /// Execute mutation on this [`QueryClient`]
    ///
    /// # Errors
    /// Will error if the mutation function errors
    ///
    /// # Panics
    /// Will always panic on engine-side
    #[cfg(target_arch = "wasm32")]
    pub async fn mutate<P, R, E, C>(
        &self,
        mutation: &Mutation<'link, P, R, E>,
        value: P,
        default_cb: Option<&MutationCallbacks<P, R, E, C>>,
        cb: Option<MutationCallbacks<P, R, E, C>>,
    ) -> Result<Rc<R>, MutateError<E>> {
        let id = mutation.inner.link.with_or_else(
            &self.inner.mutation_cache.link_target,
            || self.new_mutate_meta(mutation),
            |e| e.id,
        );

        /*use crate::mutation::MutateMeta;

        let online = crate::browser::online_handler::is_online();

        let cx = match default_cb {
            Some(cb) => match cb.on_mutate {
                Some(ref f) => f(&mut value).await,
                None => None,
            },
            None => match mutation
                .link
                .with_entry(&self.inner.link_target, |e| match e {
                    Entry::Occupied(o) => {
                        o.get().default_cb.on_mutate.as_ref().map(|f| f(&mut value))
                    }
                    Entry::Vacant(_) => None,
                }) {
                Some(f) => f.await,
                None => None,
            },
        };

        let new_data = match online {
            true => MutationData::Loading,
            false => MutationData::Err(MutateError::NoConnection),
        };

        mutation
            .link
            .with_entry(&self.inner.link_target, |e| match e {
                Entry::Occupied(mut o) => {
                    o.get_mut().data = new_data.clone();
                }
                Entry::Vacant(v) => {
                    v.insert(MutateMeta {
                        data: new_data.clone(),
                        ..MutateMeta::default()
                    });
                }
            });

        if let Some(l) = mutation.link.borrow(&self.inner.link_target) {
            for listener in &l.listeners {
                listener(new_data.clone());
            }
        }

        if !online {
            return Err(MutateError::NoConnection);
        }

        let result = mutation.execute(&value).await;

        let (result, ret) = match result {
            Ok(r) => {
                let r = Rc::new(r);
                (MutationData::Ok(Rc::clone(&r)), Ok(r))
            }
            Err(e) => {
                let e = Rc::new(e);
                (
                    MutationData::Err(MutateError::FnError(Rc::clone(&e))),
                    Err(MutateError::FnError(e)),
                )
            }
        };

        mutation
            .link
            .with_entry(&self.inner.link_target, |e| match e {
                Entry::Occupied(mut o) => {
                    o.get_mut().data = result.clone();
                }
                Entry::Vacant(v) => {
                    v.insert(MutateMeta {
                        data: result.clone(),
                        ..MutateMeta::default()
                    });
                }
            });

        for cb in [default_cb.map(|cb| &cb.inner), cb.as_ref()]
            .into_iter()
            .flatten()
        {
            let settled_ret = match ret {
                Ok(ref r) => {
                    if let Some(ref f) = cb.on_success {
                        f(Rc::clone(r), &value, &cx).await;
                    }
                    Ok(Rc::clone(r))
                }
                Err(MutateError::FnError(ref e)) => {
                    if let Some(ref f) = cb.on_error {
                        f(Rc::clone(e), &value, &cx).await;
                    }
                    Err(Rc::clone(e))
                }
                // SAFETY: `ret` never constructed with an error case other than MutateError::FnError
                Err(_) => unsafe {
                    std::hint::unreachable_unchecked();
                },
            };

            if let Some(ref f) = cb.on_settled {
                f(settled_ret, &value, &cx).await;
            }
        }

        if let Some(l) = mutation.link.borrow(&self.inner.link_target) {
            for listener in &l.listeners {
                listener(result.clone());
            }
        }
        ret*/
        todo!()
    }

    /// Get an owned copy of the the data in the client cache for the given ``query``
    #[must_use = "Has no effect other than to clone the data into an ownable type, which you should use"]
    pub fn query_data<P, R, E>(&self, query: &Query<'link, P, R, E>) -> Option<QueryData<R, E>> {
        self.inner.query_cache.data(query)
    }

    /// Fetch a query that takes no argument on this client
    #[inline]
    pub async fn fetch<R, E: Error>(&self, query: &Query<'link, (), R, E>) -> FetchResult<R, E> {
        self.fetch_with_arg(query, ()).await
    }

    /// Subscribe to updates from a client for the given [`Query`]
    pub fn subscribe_query<P, R, E>(
        &self,
        query: &Query<'link, P, R, E>,
        handler: impl Fn(QueryData<R, E>) + 'link,
    ) -> Guard<'link> {
        let handle = query.inner.link.with_or_else(
            &self.inner.query_cache.link_target,
            || self.new_fetch_meta(query),
            |value| {
                value.cache_control.set_active(true);
                value.data.add_listener(handler)
            },
        );

        Guard {
            unlisten: Box::new({
                let this = Rc::clone(&self.inner);
                let query = Rc::clone(&query.inner);
                move || {
                    query.link.with_entry(&this.query_cache.link_target, |e| {
                        if let Entry::Occupied(mut o) = e {
                            let o = o.get_mut();
                            if o.data.remove_listener(&handle) == 0 {
                                o.cache_control.set_active(false);
                            }
                        }
                    });
                }
            }),
        }
    }

    /// Subscribe to update from a client for a given [`Mutation`]
    pub fn subscribe_mutation<P, R, E, C>(
        &'link self,
        mutation: &'link Mutation<'link, P, R, E>,
        handler: impl Fn(MutationData<R, E>) + 'link,
    ) -> Guard<'link> {
        /*// TODO
        let ptr = mutation.link.with_or_else(
            &self.inner.link_target,
            || todo!(),
            |value| {
                let boxed = Box::new(handler);
                let ptr: *const () = ptr::addr_of!(*boxed).cast();
                value.listeners.insert(HashBoxPtr(boxed));
                ptr
            },
        );

        Guard {
            unlisten: Box::new(move || {
                mutation.link.with_entry(&self.inner.link_target, |e| {
                    if let Entry::Occupied(mut o) = e {
                        o.get_mut()
                            .listeners
                            .retain(|e| !ptr::eq(ptr::addr_of!(*e.0).cast(), ptr));
                    }
                });
            }),
        }*/
        todo!()
    }

    pub(crate) fn new_fetch_meta<P, R, E>(
        &self,
        query: &Query<'link, P, R, E>,
    ) -> FetchMeta<'link, R, E> {
        let cache_time: CacheTime =
            resolve::resolve_option(ConfigOption::CacheTime, &self.inner.opts, &query.inner.opts);

        FetchMeta {
            data: Listenable::new(QueryData::default()),
            id: atomic_id::next(),
            future_handles: HashSet::new(),
            cache_control: CacheControl::new(
                Rc::downgrade(&self.inner.query_cache),
                Rc::downgrade(&query.inner),
                cache_time,
            ),
        }
    }

    pub(crate) fn new_mutate_meta<P, R, E>(
        &self,
        mutation: &Mutation<'link, P, R, E>,
    ) -> MutateMeta<'link, R, E> {
        // TODO
        /*let cache_time = match self.inner.opts.mutation.cache_time {
            ConfigOpt::Inherrit => CacheTime::default(),
            ConfigOpt::Set(v) => v,
        };*/
        todo!()

        /*MutateMeta {
            data: Listenable::new(MutationData::default()),
            id: atomic_id::next(),
            cache_control: CacheControl::new(
                Rc::downgrade(&self.inner.mutation_cache),
                Rc::downgrade(&mutation.inner),
                cache_time,
            ),
        }*/
    }
}

impl<'link> QueryClientInner<'link> {
    fn fetch_with_arg_inner<P, R, E: Error>(
        self: Rc<Self>,
        query: Rc<crate::query::QueryInner<'link, P, R, E>>,
        arg: P,
        id: usize,
        count: u32,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = FetchResult<R, E>> + '_>> {
        enum Retry<T> {
            Retry(Duration),
            Return(T),
        }

        Box::pin(async move {
            #[cfg(target_arch = "wasm32")]
            let online = crate::browser::online_handler::is_online();
            #[cfg(target_arch = "wasm32")]
            let new_status = LoadingStatus::from_online(online);
            #[cfg(not(target_arch = "wasm32"))]
            let new_status = LoadingStatus::Loading;

            if query
                .link
                .with_entry(&self.query_cache.link_target, |e| match e {
                    Entry::Occupied(o) if o.get().id != id => true,
                    Entry::Vacant => true,
                    Entry::Occupied(mut o) => {
                        let entry = o.get_mut();
                        match *entry.data {
                            QueryData::Loading(ref s) if *s != new_status => {
                                Listenable::set(&mut entry.data, QueryData::Loading(new_status));
                            }
                            QueryData::Ok(_, ref s) | QueryData::Err(_, ref s)
                                if *s != new_status.as_query() =>
                            {
                                Listenable::modify(&mut entry.data, |d| match *d {
                                    QueryData::Ok(_, ref mut s) | QueryData::Err(_, ref mut s) => {
                                        *s = new_status.as_query();
                                    }
                                    QueryData::Loading(_) => unreachable!(),
                                });
                            }
                            _ => {}
                        }

                        false
                    }
                })
            {
                return FetchResult::Cancelled;
            }

            let network_mode: NetworkMode =
                resolve::resolve_option(ConfigOption::NetworkMode, &self.opts, &query.opts);

            #[cfg(target_arch = "wasm32")]
            if !online && !network_mode.should_try(count) {
                use crate::browser::online_handler::OnlineHandler;

                let no_conn = Rc::new(NoConnectionInner {
                    result: RefCell::new(None),
                    notify: Notify::new(),
                });

                let handle = future_handle::spawn_local_handle({
                    let this = Rc::clone(&self);
                    let query = Rc::clone(&query);
                    let no_conn = Rc::clone(&no_conn);
                    async move {
                        OnlineHandler::wait().await;

                        let result = match this.fetch_with_arg_inner(query, arg, id, count).await {
                            FetchResult::NoConnection(nc) => nc.wait().await,
                            FetchResult::Fresh(f) => FetchResultWaited::Fresh(f),
                            FetchResult::Stale(s) => FetchResultWaited::Stale(s),
                            FetchResult::Cancelled => FetchResultWaited::Cancelled,
                        };

                        *no_conn.result.borrow_mut() = Some(result);
                        no_conn.notify.notify_waiters();
                    }
                });

                let boxed = Box::new(handle);
                let ptr = ptr::addr_of!(boxed);
                let cleanup = boxed.cleanup();

                query
                    .link
                    .with_entry(&self.query_cache.link_target, |e| match e {
                        Entry::Occupied(mut o) => {
                            o.get_mut().future_handles.insert(HashBoxPtr(boxed));
                        }
                        Entry::Vacant => {
                            unreachable!();
                        }
                    });

                cleanup
                    .add_cleanup(move || {
                        query
                            .link
                            .with_entry(&self.query_cache.link_target, |e| match e {
                                Entry::Occupied(mut o) => {
                                    o.get_mut()
                                        .future_handles
                                        .retain(|e| !ptr::eq(ptr::addr_of!(*e.0).cast(), ptr));
                                }
                                Entry::Vacant => {}
                            });
                    })
                    .await;

                return FetchResult::NoConnection(NoConnection { inner: no_conn });
            }

            let result = query.execute_with_arg(&arg).await;
            let retry = query
                .link
                .with_entry(&self.query_cache.link_target, |e| match e {
                    Entry::Occupied(mut o) if id == o.get().id => {
                        let (result, ret) = match result {
                            Ok(r) => {
                                let r = Rc::new(r);
                                (
                                    QueryData::Ok(Rc::clone(&r), QueryStatus::Idle),
                                    Retry::Return(FetchResult::Fresh(Ok(r))),
                                )
                            }
                            Err(e) => {
                                let e = Rc::new(e);
                                let retry = resolve::resolve_retry(&self.opts, &query.opts);
                                let (status, retry) =
                                    retry.retry_delay(count, Rc::clone(&e)).map_or_else(
                                        || {
                                            (
                                                QueryStatus::Idle,
                                                Retry::Return(FetchResult::Fresh(Err(Rc::clone(
                                                    &e,
                                                )))),
                                            )
                                        },
                                        |r| (QueryStatus::Loading, Retry::Retry(r)),
                                    );
                                (QueryData::Err(Rc::clone(&e), status), retry)
                            }
                        };
                        Listenable::set(&mut o.get_mut().data, result);
                        ret
                    }
                    Entry::Occupied(_) | Entry::Vacant => Retry::Return(FetchResult::Stale(result)),
                });

            let retry = match retry {
                Retry::Return(r) => return r,
                Retry::Retry(r) => r,
            };

            sleep::sleep(retry).await;

            self.fetch_with_arg_inner(
                query,
                arg,
                id,
                count.checked_add(1).expect("retry count overflowed"),
            )
            .await
        })
    }
}

/// Guard for listener for query changes
pub struct Guard<'handle> {
    unlisten: Box<dyn Fn() + 'handle>,
}

impl Drop for Guard<'_> {
    #[inline]
    fn drop(&mut self) {
        (self.unlisten)();
    }
}

impl Debug for Guard<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Guard").finish_non_exhaustive()
    }
}
