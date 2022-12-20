#![cfg(target = "hydrate")]

use std::marker::PhantomData;

use serde::{de::DeserializeOwned, Serialize};
pub use sycamore_query_hydrate_derive::HydratableQuery;

use crate::{config::error::Error, query::Query};

/// Trait for letting structs safely create a [`HydratableQueryBuilder`]
///
/// # Safety
/// Should not be implemented manually, use ``#[derive(HydratableQuery)]`` on a unit struct to reveal the builder function
/// As the proc macro includes checks that all hydration keys are different
pub unsafe trait HydratableQuery {
    /// Parameter of query
    type Param;
    /// Successful result type of query
    type Result: Serialize + DeserializeOwned;
    /// Error result type of query
    type Error;

    /// Constructs a builder that can be used to create a hydratable query
    fn builder() -> HydratableQueryBuilder<Self::Param, Self::Result, Self::Error>;
}

type HydratableQueryPhantom<P, R, E> = PhantomData<fn() -> (P, R, E)>;

/// Allows creation of hydratable queries
#[derive(Debug)]
pub struct HydratableQueryBuilder<P, R: Serialize + DeserializeOwned, E> {
    /// Hydration key - automatically set to struct name when using [`HydratableQueryBuilder`]
    key: String,
    _phantom: HydratableQueryPhantom<P, R, E>,
}

impl<P, R: Serialize + DeserializeOwned, E: Error> HydratableQueryBuilder<P, R, E> {
    /// Creates a new [`HydratableQueryBuilder`] with a given key
    ///
    /// # Safety
    /// Use the provided ``#[hydratable_query_builder(param = P, result = <R, E>)]`` macro instead
    #[must_use = "No need to construct if you don't call build"]
    pub const unsafe fn new(key: String) -> Self {
        Self {
            key,
            _phantom: PhantomData,
        }
    }

    /// Creates a new query from the provided query, with a hydratable key
    #[must_use = "Should use return of this function to use a query with a hydration key"]
    pub fn build<'link>(&self, query: &Query<'link, P, R, E>) -> Query<'link, P, R, E> {
        Query::new_hydratable(query, self.key.clone())
    }
}
