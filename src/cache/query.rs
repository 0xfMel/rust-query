use std::{
    fmt::{self, Debug, Formatter},
    rc::Weak,
};

use crate::{
    query::{FetchMeta, Query},
    status::QueryData,
    weak_link::{Entry, Target, WeakLink},
};

use super::Cache;

/// Contains the cached data for queries in a [`QueryClient`]
pub struct QueryCache<'link> {
    pub(crate) link_target: Target<'link>,
}

impl Debug for QueryCache<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueryCache").finish_non_exhaustive()
    }
}

impl Default for QueryCache<'_> {
    fn default() -> Self {
        Self {
            link_target: Target::new(),
        }
    }
}

impl<'link, R, E> Cache<'link, FetchMeta<'link, R, E>> for Weak<QueryCache<'link>> {
    #[inline]
    fn remove_cacheable(&self, link: &WeakLink<'link, FetchMeta<'link, R, E>>) {
        if let Some(this) = self.upgrade() {
            this.remove_inner(link);
        }
    }
}

impl<'link> QueryCache<'link> {
    /// Gets the data for a given `query` in this cache
    #[inline]
    #[must_use = "Has no effect other than to clone the data into an ownable type, which you should use"]
    pub fn data<P, R, E>(&self, query: &Query<'link, P, R, E>) -> Option<QueryData<R, E>> {
        query
            .inner
            .link
            .borrow(&self.link_target)
            .map(|f| f.data.clone())
    }

    /// Removes the cached data for a given `query` from this cache
    // Caller doesn't nessassarily want the actual data, just to remove the cached value
    #[allow(clippy::must_use_candidate)]
    #[inline]
    pub fn remove_query<P, R, E>(&self, query: &Query<'link, P, R, E>) -> Option<QueryData<R, E>> {
        self.remove_inner(&query.inner.link)
    }

    #[inline]
    pub(crate) fn remove_inner<R, E>(
        &self,
        link: &WeakLink<'link, FetchMeta<'link, R, E>>,
    ) -> Option<QueryData<R, E>> {
        link.with_entry(&self.link_target, |e| match e {
            Entry::Vacant => None,
            Entry::Occupied(o) => Some(o.remove().data.unwrap()),
        })
    }
}
