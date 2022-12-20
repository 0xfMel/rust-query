use std::{
    cell::RefCell,
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    rc::Rc,
    sync::Arc,
};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use tokio::sync::Notify;

/// Fetch status of a Pending query
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum LoadingStatus {
    /// See [`QueryStatus::Loading`]
    Loading,
    /// See [`QueryStatus::Paused`]
    Paused,
}

impl LoadingStatus {
    #[allow(unreachable_code, clippy::missing_const_for_fn)]
    #[inline]
    pub(crate) fn get() -> Self {
        #[cfg(target_arch = "wasm32")]
        return match crate::browser::online_handler::is_online() {
            true => Self::Loading,
            false => Self::Paused,
        };

        Self::Loading
    }

    #[inline]
    pub(crate) const fn from_online(online: bool) -> Self {
        match online {
            true => Self::Loading,
            false => Self::Paused,
        }
    }

    #[inline]
    pub(crate) const fn as_query(self) -> QueryStatus {
        match self {
            Self::Loading => QueryStatus::Loading,
            Self::Paused => QueryStatus::Paused,
        }
    }
}

/// Fetch status of a non-pending query
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum QueryStatus {
    /// Query is currently being executed
    Loading,
    /// There is no internet connection & the query has been paused.  See [`crate::client::ClientOpts`]
    Paused,
    /// Query is not doing anything
    Idle,
}

/// The status of a [`crate::query::Query`] for a specific [`crate::client::QueryClient`], and its data or error if appliciable
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum QueryData<R, E> {
    /// There is no data available
    Loading(LoadingStatus),
    /// Query was successful
    Ok(Rc<R>, QueryStatus),
    /// Query returned an error
    Err(Rc<E>, QueryStatus),
}

impl<R, E> Clone for QueryData<R, E> {
    #[inline]
    fn clone(&self) -> Self {
        match *self {
            Self::Loading(ref s) => Self::Loading(*s),
            Self::Ok(ref r, ref s) => Self::Ok(Rc::clone(r), *s),
            Self::Err(ref e, ref s) => Self::Err(Rc::clone(e), *s),
        }
    }
}

impl<R, E> Default for QueryData<R, E> {
    #[inline]
    fn default() -> Self {
        Self::Loading(LoadingStatus::get())
    }
}

/// The status of a [`crate::mutation::Mutation`] for a specific [`crate::client::QueryClient`], and its data or error if applicable
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MutationData<R, E> {
    /// Mutation has not been initiated
    Idle,
    /// Mutation has been initiated and is executing
    Loading,
    /// Mutation was successful
    Ok(Rc<R>),
    /// Mutation returned an error
    Err(MutateError<E>),
}

impl<R, E> Default for MutationData<R, E> {
    fn default() -> Self {
        Self::Idle
    }
}

impl<R, E> Clone for MutationData<R, E> {
    fn clone(&self) -> Self {
        match *self {
            Self::Idle => Self::Idle,
            Self::Loading => Self::Loading,
            Self::Ok(ref o) => Self::Ok(Rc::clone(o)),
            Self::Err(ref e) => Self::Err(e.clone()),
        }
    }
}

/// Result of awaiting on a [`NoConnection`]
#[derive(Debug)]
pub enum FetchResultWaited<R, E> {
    /// See [`FetchResult::Fresh`]
    Fresh(Result<Rc<R>, Rc<E>>),
    /// See [`FetchResult::Stale`]
    Stale(Result<R, E>),
    /// See [`FetchResult::Cancelled`]
    Cancelled,
}

/// See [`FetchResult::NoConnection`]
#[derive(Debug)]
pub struct NoConnection<R, E> {
    pub(crate) inner: Rc<NoConnectionInner<R, E>>,
}

#[derive(Debug)]
pub(crate) struct NoConnectionInner<R, E> {
    pub(crate) result: RefCell<Option<FetchResultWaited<R, E>>>,
    pub(crate) notify: Notify,
}

impl<R, E> NoConnection<R, E> {
    /// Waits for the result of the query after connection returns
    pub async fn wait(self) -> FetchResultWaited<R, E> {
        'wait: loop {
            {
                if let Some(res) = self.inner.result.borrow_mut().take() {
                    break 'wait res;
                }
            }

            self.inner.notify.notified().await;
        }
    }
}

/// Result of a direct call to a fetch method, notifying about whether the resulting data is the latest stored in the client
#[derive(Debug)]
pub enum FetchResult<R, E> {
    /// Query was the latest to be initiated in the time it took to complete
    Fresh(Result<Rc<R>, Rc<E>>),
    /// Another query was initiated in the time it took for this query to complete
    Stale(Result<R, E>),
    /// There was no internet connection when this query was initiated
    NoConnection(NoConnection<R, E>),
    /// Another query was initated, or this query was cancelled in the time it took to retry this query
    Cancelled,
}

/// Error of a direct call to a mutate method
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MutateError<E> {
    /// The mutation function returned an error ``E``
    FnError(Arc<E>),
    /// There was no internet connection when this mutation was initiated
    NoConnection,
}

impl<E: Debug> Display for MutateError<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match *self {
            Self::FnError(ref e) => write!(f, "mutation returned an error: {:?}", e),
            Self::NoConnection => write!(f, "no internet connection when attempting mutation"),
        }
    }
}

impl<E: Debug> Error for MutateError<E> {}

impl<E: Debug> Debug for MutateError<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match *self {
            Self::FnError(ref e) => f.debug_tuple("MutateError::FnError").field(e).finish(),
            Self::NoConnection => f.debug_tuple("MutateError::NoConnection").finish(),
        }
    }
}

impl<E> Clone for MutateError<E> {
    fn clone(&self) -> Self {
        match *self {
            Self::FnError(ref e) => Self::FnError(Arc::clone(e)),
            Self::NoConnection => Self::NoConnection,
        }
    }
}
