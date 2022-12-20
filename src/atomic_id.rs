use std::sync::atomic::{AtomicUsize, Ordering};

static ID: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn next() -> usize {
    ID.fetch_add(1, Ordering::SeqCst)
}
