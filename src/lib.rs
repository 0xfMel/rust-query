// Some lints only available in nightly, want to keep the rule enabled so they go into action when stable
#![allow(unknown_lints)]
#![warn(rustdoc::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
#![warn(clippy::arithmetic_side_effects)]
#![warn(clippy::as_underscore)]
#![warn(clippy::assertions_on_result_states)]
#![warn(clippy::dbg_macro)]
#![warn(clippy::decimal_literal_representation)]
#![warn(clippy::default_numeric_fallback)]
#![warn(clippy::default_union_representation)]
#![warn(clippy::deref_by_slicing)]
#![warn(clippy::disallowed_script_idents)]
#![warn(clippy::else_if_without_else)]
#![warn(clippy::empty_drop)]
#![warn(clippy::empty_structs_with_brackets)]
#![warn(clippy::exit)]
#![warn(clippy::expect_used)]
#![warn(clippy::float_cmp_const)]
#![warn(clippy::fn_to_numeric_cast_any)]
#![warn(clippy::format_push_string)]
#![warn(clippy::get_unwrap)]
#![warn(clippy::if_then_some_else_none)]
#![warn(clippy::indexing_slicing)]
#![warn(clippy::integer_arithmetic)]
#![warn(clippy::integer_division)]
#![warn(clippy::let_underscore_must_use)]
#![warn(clippy::lossy_float_literal)]
#![warn(clippy::map_err_ignore)]
#![warn(clippy::mem_forget)]
#![warn(clippy::mixed_read_write_in_expression)]
#![warn(clippy::mod_module_files)]
#![warn(clippy::multiple_inherent_impl)]
#![warn(clippy::non_ascii_literal)]
#![warn(clippy::panic)]
#![warn(clippy::panic_in_result_fn)]
#![warn(clippy::partial_pub_fields)]
#![warn(clippy::pattern_type_mismatch)]
#![warn(clippy::print_stdout)]
#![warn(clippy::print_stderr)]
#![warn(clippy::rc_buffer)]
#![warn(clippy::rc_mutex)]
#![warn(clippy::rest_pat_in_fully_bound_structs)]
#![warn(clippy::same_name_method)]
#![warn(clippy::unseparated_literal_suffix)]
#![warn(clippy::single_char_lifetime_names)]
#![warn(clippy::str_to_string)]
#![warn(clippy::string_add)]
#![warn(clippy::string_slice)]
#![warn(clippy::string_to_string)]
#![warn(clippy::suspicious_xor_used_as_pow)]
#![warn(clippy::todo)]
#![warn(clippy::try_err)]
#![warn(clippy::undocumented_unsafe_blocks)]
#![warn(clippy::unimplemented)]
#![warn(clippy::unnecessary_self_imports)]
#![warn(clippy::unneeded_field_pattern)]
#![warn(clippy::unreachable)]
#![warn(clippy::unwrap_in_result)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::use_debug)]
#![deny(clippy::clone_on_ref_ptr)]
// Cleaner in some cases
#![allow(clippy::match_bool)]
// Intentional in most cases
#![allow(clippy::future_not_send)]

//! TODO

use std::{collections::HashSet, future::Future, pin::Pin, ptr, rc::Rc};

use instant::Instant;

use ptr_hash::HashBoxPtr;
use weak_link::{Entry, Target, WeakLink};

pub mod hydrate;
mod js_event;
mod online_handler;
mod ptr_hash;
mod weak_link;

/// Sycamore API
pub mod sycamore;

/// Setting for how [`QueryClient`] should handle being offline in the browser
pub enum NetworkMode {
    /// Only execute query when there is an internet connection, otherwise fetch status set to paused until connection returns, at which point the request is made
    Online,
    /// Ignore online status
    Always,
    /// If there is no connection, try once and pause if it fails
    OfflineFirst,
}

/// Options for [`QueryClient`]
pub struct ClientOpts {
    pub network_mode: NetworkMode,
}

impl ClientOpts {
    /// Set [`ClientOpts.network_mode`]
    #[inline]
    pub fn network_mode(&mut self, network_mode: NetworkMode) -> &mut Self {
        self.network_mode = network_mode;
        self
    }
}

impl Default for ClientOpts {
    #[inline]
    fn default() -> Self {
        Self {
            network_mode: NetworkMode::Online,
        }
    }
}

/// Fetch status of a Pending query
#[derive(Debug, Clone, Copy)]
pub enum PendingStatus {
    /// See [`FetchStatus::Fetching`]
    Fetching,
    /// See [`FetchStatus::Paused`]
    Paused,
}

/// Fetch status of a non-pending query
#[derive(Debug, Clone, Copy)]
pub enum FetchStatus {
    /// Query is currently being executed
    Fetching,
    /// There is no internet connection & the query has been paused.  See [`ClientOpts`]
    Paused,
    /// Query is not doing anything
    Idle,
}

impl PendingStatus {
    const fn as_fetch(self) -> FetchStatus {
        match self {
            Self::Fetching => FetchStatus::Fetching,
            Self::Paused => FetchStatus::Paused,
        }
    }
}

/// The status of a [`Query`] for a specific [`QueryClient`], and its data or error if appliciable
#[derive(Debug)]
pub enum QueryData<R, E> {
    /// There is no data available
    Pending(PendingStatus),
    /// Query was successful
    Ok(Rc<R>, FetchStatus),
    /// Query returned an error
    Err(Rc<E>, FetchStatus),
}

impl<R, E> Default for QueryData<R, E> {
    #[inline]
    fn default() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        return Self::Pending(PendingStatus::Fetching);

        #[cfg(target_arch = "wasm32")]
        Self::Pending(match online_handler::is_online() {
            true => PendingStatus::Fetching,
            false => PendingStatus::Paused,
        })
    }
}

impl<R, E> Clone for QueryData<R, E> {
    #[inline]
    fn clone(&self) -> Self {
        match *self {
            Self::Pending(ref s) => Self::Pending(*s),
            Self::Ok(ref r, ref s) => Self::Ok(Rc::clone(r), *s),
            Self::Err(ref e, ref s) => Self::Err(Rc::clone(e), *s),
        }
    }
}

type Listener<'func, R, E> = HashBoxPtr<dyn Fn(QueryData<R, E>) + 'func>;

struct FetchMeta<'listener, R, E> {
    data: QueryData<R, E>,
    fetched: Instant,
    listeners: HashSet<Listener<'listener, R, E>>,
}

impl<R, E> Default for FetchMeta<'_, R, E> {
    #[inline]
    fn default() -> Self {
        Self {
            data: QueryData::default(),
            fetched: Instant::now(),
            listeners: HashSet::new(),
        }
    }
}

struct QueryClientInner<'link> {
    opts: ClientOpts,
    link_target: Target<'link>,
    #[cfg(target_arch = "wasm32")]
    online_handler: online_handler::OnlineHandler<'link>,
}

/// Wrapper type for a client, allows multiple owned references to a [`QueryClient`] for the same internal state
pub struct QueryClient<'link> {
    inner: Rc<QueryClientInner<'link>>,
}

impl Clone for QueryClient<'_> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

type QueryReturn<T, E> = Pin<Box<dyn Future<Output = Result<T, E>>>>;

/// Result of a direct call to a fetch method, notifying about whether the resulting data is the latest stored in the client
#[derive(Debug)]
pub enum FetchResult<R, E> {
    Fresh(Result<Rc<R>, Rc<E>>),
    Stale(Result<R, E>),
    NoConnection,
}

impl<'link> QueryClientInner<'link> {
    async fn fetch_with_arg<P, R, E>(
        self: &Rc<Self>,
        query: &Rc<QueryInner<'link, P, R, E>>,
        arg: P,
    ) -> FetchResult<R, E> {
        #[cfg(target_arch = "wasm32")]
        let online = online_handler::is_online();

        let instant = Instant::now();
        let key = query.link.link(&self.link_target);

        let new_status = PendingStatus::Fetching;
        #[cfg(target_arch = "wasm32")]
        let new_status = match online {
            true => new_status,
            false => PendingStatus::Paused,
        };

        let new_data = query.link.with_entry(&key, |e| match e {
            Entry::Occupied(mut o) => {
                let meta = o.get_mut();
                match meta.data {
                    QueryData::Pending(ref mut s) => *s = new_status,
                    QueryData::Ok(_, ref mut s) | QueryData::Err(_, ref mut s) => {
                        *s = new_status.as_fetch();
                    }
                }
                meta.fetched = instant;
                meta.data.clone()
            }
            Entry::Vacant(v) => {
                let data = QueryData::Pending(new_status);
                v.insert(FetchMeta {
                    data: data.clone(),
                    fetched: instant,
                    listeners: HashSet::new(),
                });
                data
            }
        });

        if let Some(l) = query.link.borrow(&self.link_target) {
            for listener in &l.listeners {
                listener(new_data.clone());
            }
        }

        #[cfg(target_arch = "wasm32")]
        if !online {
            let key = self
                .online_handler
                .link
                .link(&query.online_handler_link_target);
            self.online_handler.link.insert(
                &key,
                Box::pin({
                    let this = Rc::downgrade(self);
                    let query = Rc::downgrade(query);

                    async move {
                        if let (Some(this), Some(query)) = (this.upgrade(), query.upgrade()) {
                            this.fetch_with_arg(&query, arg).await;
                        }
                    }
                }),
            );

            return FetchResult::NoConnection;
        }

        let result = query.execute_with_arg(arg).await;
        let mut new_data = None;

        let ret = query.link.with_entry(&key, |e| match e {
            Entry::Occupied(mut o) if instant >= o.get().fetched => {
                let (result, ret) = match result {
                    Ok(r) => {
                        let r = Rc::new(r);
                        (QueryData::Ok(Rc::clone(&r), FetchStatus::Idle), Ok(r))
                    }
                    Err(e) => {
                        let e = Rc::new(e);
                        (QueryData::Err(Rc::clone(&e), FetchStatus::Idle), Err(e))
                    }
                };

                new_data = Some(result.clone());
                let meta = o.get_mut();
                meta.data = result;

                FetchResult::Fresh(ret)
            }
            Entry::Occupied(_) | Entry::Vacant(_) => FetchResult::Stale(result),
        });

        if let Some(new_data) = new_data {
            if let Some(l) = query.link.borrow(&self.link_target) {
                for listener in &l.listeners {
                    listener(new_data.clone());
                }
            }
        }
        ret
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

impl<'link> QueryClient<'link> {
    /// Create a new [`QueryClient`] with provided options
    #[must_use]
    #[inline]
    pub fn new(opts: ClientOpts) -> Self {
        Self {
            inner: Rc::new(QueryClientInner {
                opts,
                link_target: Target::new(),
                #[cfg(target_arch = "wasm32")]
                online_handler: online_handler::OnlineHandler::new(),
            }),
        }
    }

    /// Get an owned copy of the the data in the client cache for the given ``query``
    #[must_use = "Has no effect other than to clone the result into an ownable type, which you should use"]
    pub fn get_result<P, R, E>(&self, query: &Query<'link, P, R, E>) -> Option<QueryData<R, E>> {
        query
            .inner
            .link
            .borrow(&self.inner.link_target)
            .map(|f| f.data.clone())
    }

    /// Fetch a query that takes no argument on this client
    #[inline]
    pub async fn fetch<R, E>(&self, query: &Query<'link, (), R, E>) -> FetchResult<R, E> {
        self.fetch_with_arg(query, ()).await
    }

    /// Fetch a query that takes an argument ``P`` on this client
    #[inline]
    pub async fn fetch_with_arg<P: 'link, R, E>(
        &self,
        query: &Query<'link, P, R, E>,
        arg: P,
    ) -> FetchResult<R, E> {
        let fetch = self.inner.fetch_with_arg(&query.inner, arg);
        fetch.await
    }

    /// TODO
    pub async fn prefetch<R, E>(&self, query: &Query<'link, (), R, E>) {
        self.fetch(query).await;
    }

    /// TODO
    pub async fn prefetch_with_arg<P: 'link, R, E>(&self, query: &Query<'link, P, R, E>, arg: P) {
        self.fetch_with_arg(query, arg).await;
    }

    /// Subscribe to updates from a client for the given ``query``
    pub fn subscribe<P, R, E>(
        &self,
        query: &Query<'link, P, R, E>,
        handler: impl Fn(QueryData<R, E>) + 'link,
    ) -> Guard<'link> {
        let key = query.inner.link.link(&self.inner.link_target);
        let ptr = query.inner.link.with_or_default(&key, |value| {
            let boxed = Box::new(handler);
            let ptr: *const () = ptr::addr_of!(*boxed).cast();
            value.listeners.insert(HashBoxPtr(boxed));
            ptr
        });

        Guard {
            unlisten: Box::new({
                let query = query.clone();
                move || {
                    query.inner.link.with_entry(&key, |e| {
                        if let Entry::Occupied(mut o) = e {
                            o.get_mut()
                                .listeners
                                .retain(|e| !std::ptr::eq(ptr::addr_of!(*e.0).cast(), ptr));
                        }
                    });
                }
            }),
        }
    }
}

impl Default for QueryClient<'_> {
    #[inline]
    fn default() -> Self {
        Self::new(ClientOpts::default())
    }
}

struct QueryInner<'link, P: 'link, R, E> {
    func: QueryFn<P, R, E>,
    link: WeakLink<'link, FetchMeta<'link, R, E>>,
    #[cfg(target_arch = "wasm32")]
    online_handler_link_target: Target<'link>,
}

/// Wrapper type for a query, allowing for multiple owned references to the same state
pub struct Query<'link, P: 'link, R, E> {
    inner: Rc<QueryInner<'link, P, R, E>>,
    hydrate_key: Option<String>,
}

impl<P, R, E> Clone for Query<'_, P, R, E> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
            hydrate_key: self.hydrate_key.clone(),
        }
    }
}

enum QueryFn<P, R, E> {
    NoParam(Box<dyn Fn() -> QueryReturn<R, E>>),
    WithParam(Box<dyn Fn(P) -> QueryReturn<R, E>>),
}

impl<R, E> Query<'_, (), R, E> {
    /// Create new [`Query`] with no arguments
    #[must_use = "No reason to create a Query if you don't use it"]
    #[inline]
    pub fn new(func: Box<dyn Fn() -> QueryReturn<R, E>>) -> Self {
        Self::new_inner(QueryFn::NoParam(func))
    }

    /// Directly execute query without a client
    ///
    /// # Errors
    /// Will error if the provided query function does
    #[inline]
    pub async fn execute(&self) -> Result<R, E> {
        match self.inner.func {
            QueryFn::NoParam(ref func) => func(),
            QueryFn::WithParam(ref func) => func(()),
        }
        .await
    }
}

impl<P, R, E> QueryInner<'_, P, R, E> {
    /// Directly execute query without a client
    ///
    /// # Errors
    /// Will error if the provided query function does
    #[inline]
    pub async fn execute_with_arg(self: &Rc<Self>, arg: P) -> Result<R, E> {
        match self.func {
            QueryFn::NoParam(ref func) => func(),
            QueryFn::WithParam(ref func) => func(arg),
        }
        .await
    }
}

impl<P, R, E> Query<'_, P, R, E> {
    #[inline]
    fn new_inner(func: QueryFn<P, R, E>) -> Self {
        Self {
            inner: Rc::new(QueryInner {
                func,
                link: WeakLink::new(),
                #[cfg(target_arch = "wasm32")]
                online_handler_link_target: Target::new(),
            }),
            hydrate_key: None,
        }
    }

    /// Create a new [`Query`] with an argument of type ``P``
    #[must_use = "No reason to create a Query if you don't use it"]
    #[inline]
    pub fn new_with_param(func: Box<dyn Fn(P) -> QueryReturn<R, E>>) -> Self {
        Self::new_inner(QueryFn::WithParam(func))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]

    use super::*;

    fn check<E>(res: FetchResult<i32, E>, exp: i32) {
        match res {
            FetchResult::Fresh(Ok(f)) => assert_eq!(exp, *f),
            _ => panic!(),
        }
    }

    #[tokio::test]
    async fn multiple_client_query() {
        let client1 = QueryClient::default();
        let client2 = QueryClient::default();
        let query1 = Query::new(Box::new(|| Box::pin(async { Ok::<i32, ()>(12345_i32) })));
        let query2 = Query::new(Box::new(|| Box::pin(async { Ok::<i32, ()>(67890_i32) })));

        check(client1.fetch(&query1).await, 12345_i32);
        check(client2.fetch(&query1).await, 12345_i32);
        check(client1.fetch(&query2).await, 67890_i32);
        check(client2.fetch(&query2).await, 67890_i32);
    }
}
