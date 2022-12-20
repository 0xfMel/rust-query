use std::{
    fmt::{self, Debug, Formatter},
    rc::Weak,
};

use crate::{
    mutation::{MutateMeta, Mutation},
    status::MutationData,
    weak_link::{Entry, Target, WeakLink},
};

use super::Cache;

/// Contains the cached data for mutations in a [`QueryClient`]
pub struct MutationCache<'link> {
    pub(crate) link_target: Target<'link>,
}

impl<'link, R, E> Cache<'link, MutateMeta<'link, R, E>> for Weak<MutationCache<'link>> {
    fn remove_cacheable(&self, link: &WeakLink<'link, MutateMeta<'link, R, E>>) {
        if let Some(this) = self.upgrade() {
            this.remove_inner(link);
        }
    }
}

impl Debug for MutationCache<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("MutationCache").finish_non_exhaustive()
    }
}

impl Default for MutationCache<'_> {
    fn default() -> Self {
        Self {
            link_target: Target::new(),
        }
    }
}

impl<'link> MutationCache<'link> {
    /// Gets the data for a given `mutation` in this cache
    #[inline]
    #[must_use = "Has no effect other than to clone the data into an ownable type, which you should use"]
    pub fn data<P, R, E>(&self, mutation: &Mutation<'link, P, R, E>) -> Option<MutationData<R, E>> {
        mutation
            .inner
            .link
            .borrow(&self.link_target)
            .map(|f| f.data.clone())
    }

    /// Removes the cached data for a given `mutation` from this cache
    // Caller doesn't nessassarily want the actual data, just to remove the cached value
    #[allow(clippy::must_use_candidate)]
    #[inline]
    pub fn remove_mutation<P, R, E>(
        &self,
        mutation: &Mutation<'link, P, R, E>,
    ) -> Option<MutationData<R, E>> {
        self.remove_inner(&mutation.inner.link)
    }

    #[inline]
    pub(crate) fn remove_inner<R, E>(
        &self,
        link: &WeakLink<'link, MutateMeta<'link, R, E>>,
    ) -> Option<MutationData<R, E>> {
        link.with_entry(&self.link_target, |e| match e {
            Entry::Vacant => None,
            Entry::Occupied(o) => Some(o.remove().data.unwrap()),
        })
    }
}
