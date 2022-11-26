use std::marker::PhantomData;

pub use sycamore_query_hydrate_derive::HydratableQuery;

use crate::Query;

/// Trait for letting structs safely create a [`HydratableQueryBuilder`]
///
/// # Safety
/// Should not be implemented manually, use ``#[derive(HydratableQuery)]`` on a unit struct to reveal the builder function
/// As the proc macro includes checks that all hydration keys are different
pub unsafe trait HydratableQuery {
    /// Parameter of query
    type Param;
    /// Successful result type of query
    type Result;
    /// Error result type of query
    type Error;

    /// Constructs a builder that can be used to create a hydratable query
    fn builder() -> HydratableQueryBuilder<Self::Param, Self::Result, Self::Error>;
}

/// Allows creation of hydratable queries
pub struct HydratableQueryBuilder<P, R, E> {
    /// Hydration key - automatically set to struct name when using [`HydratableQueryBuilder`]
    key: String,
    /// Covariant, doesn't drop P
    _p: PhantomData<fn() -> P>,
    /// Covariant, doesn't drop R
    _r: PhantomData<fn() -> R>,
    /// Covariant, doesn't drop E
    _e: PhantomData<fn() -> E>,
}

impl<P, R, E> HydratableQueryBuilder<P, R, E> {
    /// Creates a new [`HydratableQueryBuilder`] with a given key
    ///
    /// # Safety
    /// Use the provided ``#[hydratable_query_builder(param = P, result = <R, E>)]`` macro instead
    #[must_use = "No need to construct if you don't call build"]
    pub const unsafe fn new(key: String) -> Self {
        Self {
            key,
            _p: PhantomData,
            _r: PhantomData,
            _e: PhantomData,
        }
    }

    /// Creates a new query wrapper as a copy of the provided query
    /// Note: Keeps the same internal state, will be considered as the same query by a [`crate::QueryClient`]
    #[must_use = "Should use return of this function to use a query with a hydration key"]
    pub fn build<'link>(&self, query: &Query<'link, P, R, E>) -> Query<'link, P, R, E> {
        let mut query = query.clone();
        query.hydrate_key = Some(self.key.clone());
        query
    }
}
