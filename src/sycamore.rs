use std::{mem, rc::Rc};

use sycamore::prelude::*;

use crate::{Query, QueryClient, QueryData};

struct ClientPtr(*const QueryClient<'static>);

pub fn provide_query_client<'a>(cx: Scope<'a>, client: &QueryClient<'a>) -> &'a QueryClient<'a> {
    let ref_ = create_ref(cx, client.clone());
    provide_context(cx, ClientPtr(unsafe { mem::transmute(ref_) }));
    ref_
}

pub fn use_query_client<'a>(cx: Scope<'a>) -> &QueryClient<'a> {
    unsafe {
        &*mem::transmute::<_, *const QueryClient<'a>>(
            try_use_context::<ClientPtr>(cx)
                .expect("query client should be provided in a this scope or higher")
                .0,
        )
    }
}

pub fn use_query<'a, R, E>(
    cx: Scope<'a>,
    query: &Query<'a, (), R, E>,
) -> &'a Signal<QueryData<R, E>> {
    use_query_with_arg(cx, query, ())
}

pub fn use_query_with_arg<'a, P, R, E>(
    cx: Scope<'a>,
    query: &Query<'a, P, R, E>,
    arg: P,
) -> &'a Signal<QueryData<R, E>> {
    let data_signal = create_signal(cx, QueryData::default());
    use_query_inner(cx, data_signal, query, arg)
}

fn use_query_inner<'a, P, R, E>(
    cx: Scope<'a>,
    data_signal: &'a Signal<QueryData<R, E>>,
    query: &Query<'a, P, R, E>,
    arg: P,
) -> &'a Signal<QueryData<R, E>> {
    #[cfg(target_arch = "wasm32")]
    {
        let query = create_ref(cx, query.clone());
        let client = use_query_client(cx);
        let guard = client.subscribe(query, |data| {
            data_signal.set(data);
        });
        create_ref(cx, guard);
        /*perseus::spawn_local_scoped(cx, async {
            client.fetch_with_arg(query, arg).await;
        });*/
    }
    data_signal
}

pub fn use_query_with_signal_rc_arg<'a, P, R, E>(
    cx: Scope<'a>,
    query: &'a Query<'a, Rc<P>, R, E>,
    arg: &'a Signal<P>,
) -> &'a Signal<QueryData<R, E>> {
    let data_signal = create_signal(cx, QueryData::default());
    create_effect(cx, move || {
        use_query_inner(cx, &data_signal, query, arg.get());
    });
    data_signal
}

pub fn use_query_with_signal_arg<'a, P: Clone, R, E>(
    cx: Scope<'a>,
    query: &'a Query<'a, P, R, E>,
    arg: &'a Signal<P>,
) -> &'a Signal<QueryData<R, E>> {
    let data_signal = create_signal(cx, QueryData::default());
    create_effect(cx, move || {
        use_query_inner(cx, &data_signal, query, arg.get().as_ref().clone());
    });
    data_signal
}
