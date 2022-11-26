use std::{mem, rc::Rc};

use sycamore::prelude::*;

use crate::{Query, QueryClient, QueryData};

/// Type of a raw pointer to a client, so it can be identified by sycamore's context system
/// Raw pointer required to erase lifetime because context values must be 'static
struct ClientPtr(*const QueryClient<'static>);

/// Provide [`QueryClient`] context that can be used in this scope & any child scopes
///
/// # Panics
/// Will panic if already called in this scope
/// Note that new components don't mean new scopes, new scopes are created explicitly
///
// Just returning a reference to the same client with an extended lifetime, don't need to be used
#[allow(clippy::must_use_candidate)]
pub fn provide_query_client<'scope, 'link>(
    cx: Scope<'scope>,
    client: &QueryClient<'link>,
) -> &'scope QueryClient<'link>
where
    'link: 'scope,
{
    let ref_ = create_ref(cx, client.clone());
    // Lifetime erasure
    // SAFETY: The above create_ref means ref_ lives as long as the scope, and the value can only be accessed if the scope still exists
    provide_context(cx, ClientPtr(unsafe { mem::transmute(ref_) }));
    ref_
}

/// Get a reference to the client provided in this scope or one of its parents
///
/// # Panics
/// Will panic if a client hasn't been provided in this scope or one of its parents
///
// For consistent panicing behaviour with other contexts
#[allow(clippy::expect_used)]
#[must_use = "Function has no other effect than to provide reference to the QueryClient, it should be used"]
pub fn use_query_client<'scope>(cx: Scope<'scope>) -> &QueryClient<'scope> {
    // SAFETY: If the context exists, the client still exists
    unsafe {
        // Required for lifetime erasure
        #[allow(clippy::transmute_ptr_to_ptr)]
        &*mem::transmute::<_, *const QueryClient<'scope>>(
            try_use_context::<ClientPtr>(cx)
                .expect("query client should be provided in a this scope or higher")
                .0
                .cast::<QueryClient>(),
        )
    }
}

/// Get the cached query data, or initiate a fetch for the data, returning a reactive signal of the status & result
#[must_use = "If you don't need the query result, consider QueryClient::prefetch"]
#[inline]
pub fn use_query<'scope, R, E>(
    cx: Scope<'scope>,
    query: &Query<'scope, (), R, E>,
) -> &'scope Signal<QueryData<R, E>> {
    use_query_with_arg(cx, query, ())
}

/// Get the cached query data, or initiate a fetch for the data, returning a reactive signal of the status & result
#[must_use = "If you don't need the query result, consider QueryClient::prefetch"]
#[inline]
pub fn use_query_with_arg<'scope, P, R, E>(
    cx: Scope<'scope>,
    query: &Query<'scope, P, R, E>,
    arg: P,
) -> &'scope Signal<QueryData<R, E>> {
    let data_signal = create_signal(cx, QueryData::default());
    use_query_inner(cx, data_signal, query, arg)
}

/// Helper function for listening to changes to a query for the given client and updating the reactive signal, and for executing the query
#[inline]
fn use_query_inner<'scope, P, R, E>(
    cx: Scope<'scope>,
    data_signal: &'scope Signal<QueryData<R, E>>,
    query: &Query<'scope, P, R, E>,
    arg: P,
) -> &'scope Signal<QueryData<R, E>> {
    //#[cfg(target_arch = "wasm32")]
    {
        use sycamore::futures;

        let query = create_ref(cx, query.clone());
        let client = use_query_client(cx);
        let guard = client.subscribe(query, |data| {
            data_signal.set(data);
        });
        create_ref(cx, guard);
        futures::spawn_local_scoped(cx, async {
            client.fetch_with_arg(query, arg).await;
        });
    }
    data_signal
}

/// Get the cached query data, or initiate a fetch for the data, returning a reactive signal of the status & result
/// Accepts a signal as the arg, which it will create an effect on, executing the query again if it changes
/// Only for Quries that take an Rc argument
#[must_use = "If you don't need the query result, consider QueryClient::prefetch"]
#[inline]
pub fn use_query_with_signal_rc_arg<'scope, P, R, E>(
    cx: Scope<'scope>,
    query: &'scope Query<'scope, Rc<P>, R, E>,
    arg: &'scope Signal<P>,
) -> &'scope Signal<QueryData<R, E>> {
    let data_signal = create_signal(cx, QueryData::default());
    create_effect(cx, move || {
        use_query_inner(cx, data_signal, query, arg.get());
    });
    data_signal
}

/// Get the cached query data, or initiate a fetch for the data, returning a reactive signal of the status & result
/// Accepts a signal as the arg, which it will create an effect on, executing the query again if it changes
/// Clones the value inside the signal
#[must_use = "If you don't need the query result, consider QueryClient::prefetch"]
#[inline]
pub fn use_query_with_signal_arg<'scope, P: Clone, R, E>(
    cx: Scope<'scope>,
    query: &'scope Query<'scope, P, R, E>,
    arg: &'scope Signal<P>,
) -> &'scope Signal<QueryData<R, E>> {
    let data_signal = create_signal(cx, QueryData::default());
    create_effect(cx, move || {
        use_query_inner(cx, data_signal, query, arg.get().as_ref().clone());
    });
    data_signal
}
