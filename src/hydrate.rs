use std::marker::PhantomData;

pub use sycamore_query_hydrate_derive::HydratableQuery;

use crate::Query;

pub trait HydratableQuery {
    type Param;
    type Result;
    type Error;

    fn builder() -> HydratableQueryBuilder<Self::Param, Self::Result, Self::Error>;
}

pub struct HydratableQueryBuilder<P, R, E> {
    key: String,
    _p: PhantomData<*const P>,
    _r: PhantomData<*const R>,
    _e: PhantomData<*const E>,
}

// SAFETY: Use the provided #[hydratable_query_builder(param = P, result = <R, E>)] macro instead
// which ensures type safety for hydration on the browser
impl<P, R, E> HydratableQueryBuilder<P, R, E> {
    pub unsafe fn new(key: String) -> Self {
        Self {
            key,
            _p: PhantomData,
            _r: PhantomData,
            _e: PhantomData,
        }
    }

    pub fn build<'a>(&self, mut query: Query<'a, P, R, E>) -> Query<'a, P, R, E> {
        query.hydrate_key = Some(self.key.clone());
        query
    }
}
