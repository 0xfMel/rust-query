use std::{collections::HashSet, future::Future, pin::Pin, rc::Rc};

use instant::Instant;
use online_handler::OnlineHandler;
use ptr_hash::BoxPtrHash;
use weak_link::{Entry, WeakLink, WeakLinkTarget};

mod hydrate;
mod js_event;
mod online_handler;
mod ptr_hash;
mod test;
mod weak_link;

pub mod sycamore;

pub enum NetworkMode {
    Online,
    Always,
    OfflineFirst,
}

pub struct ClientOpts {
    network_mode: NetworkMode,
}

impl ClientOpts {
    pub fn network_mode(&mut self, network_mode: NetworkMode) -> &mut Self {
        self.network_mode = network_mode;
        self
    }
}

impl Default for ClientOpts {
    fn default() -> Self {
        Self {
            network_mode: NetworkMode::Online,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PendingStatus {
    Fetching,
    Paused,
}

#[derive(Debug, Clone, Copy)]
pub enum FetchStatus {
    Fetching,
    Paused,
    Idle,
}

impl PendingStatus {
    fn as_fetch(&self) -> FetchStatus {
        match self {
            PendingStatus::Fetching => FetchStatus::Fetching,
            PendingStatus::Paused => FetchStatus::Paused,
        }
    }
}

#[derive(Debug)]
pub enum QueryData<R, E> {
    Pending(PendingStatus),
    Ok(Rc<R>, FetchStatus),
    Err(Rc<E>, FetchStatus),
}

impl<R, E> Default for QueryData<R, E> {
    fn default() -> Self {
        Self::Pending(match OnlineHandler::is_online() {
            true => PendingStatus::Fetching,
            false => PendingStatus::Paused,
        })
    }
}

impl<R, E> Clone for QueryData<R, E> {
    fn clone(&self) -> Self {
        match self {
            QueryData::Pending(s) => QueryData::Pending(*s),
            QueryData::Ok(r, s) => QueryData::Ok(Rc::clone(r), *s),
            QueryData::Err(e, s) => QueryData::Err(Rc::clone(e), *s),
        }
    }
}

type Listener<'a, R, E> = BoxPtrHash<dyn Fn(QueryData<R, E>) + 'a>;

struct FetchMeta<'a, R, E> {
    data: QueryData<R, E>,
    fetched: Instant,
    listeners: HashSet<Listener<'a, R, E>>,
}

impl<R, E> Default for FetchMeta<'_, R, E> {
    fn default() -> Self {
        Self {
            data: QueryData::default(),
            fetched: Instant::now(),
            listeners: HashSet::new(),
        }
    }
}

struct QueryClientInner<'a> {
    opts: ClientOpts,
    link_target: WeakLinkTarget<'a>,
    online_handler: OnlineHandler<'a>,
}

pub struct QueryClient<'a> {
    inner: Rc<QueryClientInner<'a>>,
}

impl Clone for QueryClient<'_> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

type QueryReturn<T, E> = Pin<Box<dyn Future<Output = Result<T, E>>>>;

#[derive(Debug)]
pub enum FetchResult<R, E> {
    Fresh(Result<Rc<R>, Rc<E>>),
    Stale(Result<R, E>),
    NoConnection,
}

trait ReallyAny {}
impl<T> ReallyAny for T {}

impl<'a> QueryClientInner<'a> {
    async fn fetch_with_arg<P, R, E>(
        self: &Rc<Self>,
        query: &Rc<QueryInner<'a, P, R, E>>,
        arg: P,
    ) -> FetchResult<R, E> {
        let online = OnlineHandler::is_online();
        let instant = Instant::now();
        let key = query.link.link(&self.link_target);

        let new_status = match online {
            true => PendingStatus::Fetching,
            false => PendingStatus::Paused,
        };

        let new_data = query.link.with_entry(&key, |e| match e {
            Entry::Occupied(mut o) => {
                let meta = o.get_mut();
                match meta.data {
                    QueryData::Pending(ref mut s) => *s = new_status,
                    QueryData::Ok(_, ref mut s) | QueryData::Err(_, ref mut s) => {
                        *s = new_status.as_fetch()
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
            for listener in l.listeners.iter() {
                listener(new_data.clone());
            }
        }

        if !online {
            let key = self.online_handler.link.link(&self.link_target);
            self.online_handler.link.insert(
                &key,
                Box::new({
                    let this = Rc::downgrade(self);
                    let query = Rc::downgrade(query);

                    move || {
                        Box::pin(async move {
                            if let (Some(this), Some(query)) = (this.upgrade(), query.upgrade()) {
                                this.fetch_with_arg(&query, arg).await;
                            }
                        })
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
                for listener in l.listeners.iter() {
                    listener(new_data.clone());
                }
            }
        }
        ret
    }
}

pub struct Guard<'a> {
    unlisten: Box<dyn Fn() + 'a>,
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        (self.unlisten)();
    }
}

impl<'a> QueryClient<'a> {
    pub fn new(opts: ClientOpts) -> Self {
        Self {
            inner: Rc::new(QueryClientInner {
                opts,
                link_target: WeakLinkTarget::new(),
                online_handler: OnlineHandler::new(),
            }),
        }
    }

    pub fn get_result<P, R, E>(&self, query: &Query<'a, P, R, E>) -> Option<QueryData<R, E>> {
        query
            .inner
            .link
            .borrow(&self.inner.link_target)
            .map(|f| f.data.clone())
    }

    pub async fn fetch<R, E>(&self, query: &Query<'a, (), R, E>) -> FetchResult<R, E> {
        self.fetch_with_arg(query, ()).await
    }

    pub async fn fetch_with_arg<P: 'a, R, E>(
        &self,
        query: &Query<'a, P, R, E>,
        arg: P,
    ) -> FetchResult<R, E> {
        self.inner.fetch_with_arg(&query.inner, arg).await
    }

    pub async fn prefetch<R, E>(&self, query: &Query<'a, (), R, E>) {
        self.fetch(query).await;
    }

    pub async fn prefetch_with_arg<P: 'a, R, E>(&self, query: &Query<'a, P, R, E>, arg: P) {
        self.fetch_with_arg(query, arg).await;
    }

    pub fn subscribe<P, R, E>(
        &self,
        query: &Query<'a, P, R, E>,
        handler: impl Fn(QueryData<R, E>) + 'a,
    ) -> Guard<'a> {
        let key = query.inner.link.link(&self.inner.link_target);
        let ptr = query.inner.link.with_or_default(&key, |value| {
            let boxed = Box::new(handler);
            let ptr = &*boxed as *const dyn Fn(QueryData<R, E>) as *const ();
            value.listeners.insert(BoxPtrHash(boxed));
            ptr
        });

        Guard {
            unlisten: Box::new({
                let query = query.clone();
                move || {
                    query.inner.link.with_entry(&key, |e| {
                        if let Entry::Occupied(mut o) = e {
                            o.get_mut().listeners.retain(|e| {
                                !std::ptr::eq(
                                    &*e.0 as *const dyn Fn(QueryData<R, E>) as *const (),
                                    ptr,
                                )
                            })
                        }
                    })
                }
            }),
        }
    }
}

impl Default for QueryClient<'_> {
    fn default() -> Self {
        Self::new(ClientOpts::default())
    }
}

struct QueryInner<'a, P: 'a, R, E> {
    func: QueryFn<P, R, E>,
    link: WeakLink<'a, FetchMeta<'a, R, E>>,
}

pub struct Query<'a, P: 'a, R, E> {
    inner: Rc<QueryInner<'a, P, R, E>>,
    hydrate_key: Option<String>,
}

impl<P, R, E> Clone for Query<'_, P, R, E> {
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
    pub fn new(func: Box<dyn Fn() -> QueryReturn<R, E>>) -> Self {
        Self::new_inner(QueryFn::NoParam(func))
    }

    pub async fn execute(&self) -> Result<R, E> {
        use QueryFn::*;
        match &self.inner.func {
            NoParam(func) => func(),
            WithParam(func) => func(()),
        }
        .await
    }
}

impl<P, R, E> QueryInner<'_, P, R, E> {
    async fn execute_with_arg(self: &Rc<Self>, arg: P) -> Result<R, E> {
        use QueryFn::*;
        match &self.func {
            NoParam(func) => func(),
            WithParam(func) => func(arg),
        }
        .await
    }
}

impl<'a, P, R, E> Query<'a, P, R, E> {
    fn new_inner(func: QueryFn<P, R, E>) -> Self {
        Self {
            inner: Rc::new(QueryInner {
                func,
                link: WeakLink::new(),
            }),
            hydrate_key: None,
        }
    }

    pub fn new_with_param(func: Box<dyn Fn(P) -> QueryReturn<R, E>>) -> Self {
        Self::new_inner(QueryFn::WithParam(func))
    }
}
