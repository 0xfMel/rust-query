use std::{cell::Cell, rc::Rc};

use tokio::{select, sync::Notify};

use crate::{
    config::CacheTime,
    futures::future_handle,
    listenable::{Listenable, Listener},
    sleep,
    weak_link::WeakLink,
};

/// Cache for mutations
pub mod mutation;
/// Cache for queries
pub mod query;

pub(crate) trait Cache<'link, T> {
    fn remove_cacheable(&self, link: &WeakLink<'link, T>);
}

pub(crate) trait Cacheable<'link> {
    type LinkData;

    fn link(&self) -> Option<WeakLink<'link, Self::LinkData>>;
}

pub(crate) struct CacheControl<'func> {
    active: Listenable<'func, bool>,
}

impl<'link> CacheControl<'link> {
    pub(crate) fn new<T: 'link>(
        cache: impl Cache<'link, T> + Clone + 'link,
        cacheable: impl Cacheable<'link, LinkData = T> + Clone + 'link,
        cache_time: CacheTime,
    ) -> Self {
        let mut this = Self {
            active: Listenable::new(false),
        };

        let CacheTime::Duration(dur) = cache_time else {
            return this;
        };

        let notify = Rc::new(Notify::new());
        let active = Rc::new(Cell::new(false));
        let fut_handle = Rc::new(Cell::new(None));
        let handle_active = {
            let fut_handle = Rc::clone(&fut_handle);
            move |new_active| {
                active.set(new_active);
                if new_active {
                    notify.notify_waiters();
                    return;
                }

                let handle = future_handle::spawn_local_handle({
                    let notify = Rc::clone(&notify);
                    let active = Rc::clone(&active);
                    let cache = cache.clone();
                    let cacheable = cacheable.clone();
                    async move {
                        if active.get() {
                            return;
                        }
                        let sleep = sleep::sleep(dur);
                        tokio::pin!(sleep);

                        while !active.get() {
                            select! {
                                _ = &mut sleep => {
                                    if let Some(link) = cacheable.link() {
                                        cache.remove_cacheable(&link);
                                    }
                                    break;
                                }
                                _ = notify.notified() => {
                                    // Cancelled
                                }
                            }
                        }
                    }
                });
                fut_handle.set(Some(handle));
            }
        };

        handle_active(false);
        this.active.add_listener_direct(Listener {
            f: Box::new(handle_active),
            drop_f: Some(Box::new(move || drop(fut_handle))),
        });
        this
    }

    pub(crate) fn active(&self) -> bool {
        *self.active
    }

    pub(crate) fn set_active(&mut self, active: bool) {
        Listenable::set_cmp(&mut self.active, active);
    }
}
