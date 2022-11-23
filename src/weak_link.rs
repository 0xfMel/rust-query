use std::{
    cell::{Ref, RefCell},
    collections::{
        hash_map::{
            Entry as HashMapEntry, OccupiedEntry as HashMapOccupied, VacantEntry as HashMapVacant,
        },
        HashMap, HashSet,
    },
    rc::{Rc, Weak},
};

use crate::ptr_hash::WeakPtrHash;

pub(crate) struct WeakLink<'a, T: 'a> {
    inner: Rc<WeakLinkInner<'a, T>>,
}

pub(crate) struct WeakLinkKey<'a>(WeakPtrHash<WeakLinkTargetInner<'a>>);

pub(crate) enum Entry<'a, 'b, T> {
    Vacant(VacantEntry<'a, 'b, T>),
    Occupied(OccupiedEntry<'a, 'b, T>),
}

pub(crate) struct VacantEntry<'a, 'b, T> {
    entry: HashMapVacant<'a, WeakPtrHash<WeakLinkTargetInner<'b>>, T>,
}

pub(crate) struct OccupiedEntry<'a, 'b, T> {
    entry: HashMapOccupied<'a, WeakPtrHash<WeakLinkTargetInner<'b>>, T>,
}

impl<'a, T> VacantEntry<'a, '_, T> {
    pub(crate) fn insert(self, value: T) -> &'a mut T {
        self.entry.insert(value)
    }
}

impl<T> OccupiedEntry<'_, '_, T> {
    pub(crate) fn get(&self) -> &T {
        self.entry.get()
    }

    pub(crate) fn get_mut(&mut self) -> &mut T {
        self.entry.get_mut()
    }
}

impl<'a, T: Default> WeakLink<'a, T> {
    pub(crate) fn with_or_default<R>(
        &self,
        key: &WeakLinkKey<'a>,
        f: impl FnOnce(&mut T) -> R,
    ) -> R {
        let mut targets = self.inner.targets.borrow_mut();
        let value = targets
            .entry(WeakPtrHash::clone(&key.0))
            .or_insert_with(T::default);
        f(value)
    }
}

impl<'a, T> WeakLink<'a, T> {
    pub(crate) fn new() -> Self {
        Self {
            inner: Rc::new(WeakLinkInner {
                targets: RefCell::new(HashMap::new()),
            }),
        }
    }

    pub(crate) fn link(&self, target: &WeakLinkTarget<'a>) -> WeakLinkKey<'a> {
        target.inner.links.borrow_mut().insert(WeakPtrHash(
            Rc::downgrade(&self.inner) as Weak<dyn WeakLinkFrom>
        ));

        WeakLinkKey(WeakPtrHash(Rc::downgrade(&target.inner)))
    }

    pub(crate) fn with_entry<R>(
        &self,
        key: &WeakLinkKey<'a>,
        f: impl FnOnce(Entry<'_, 'a, T>) -> R,
    ) -> R {
        let mut targets = self.inner.targets.borrow_mut();
        let entry = targets.entry(WeakPtrHash::clone(&key.0));
        f(match entry {
            HashMapEntry::Occupied(o) => Entry::Occupied(OccupiedEntry { entry: o }),
            HashMapEntry::Vacant(v) => Entry::Vacant(VacantEntry { entry: v }),
        })
    }

    pub(crate) fn insert(&self, key: &WeakLinkKey<'a>, value: T) {
        self.inner
            .targets
            .borrow_mut()
            .insert(WeakPtrHash::clone(&key.0), value);
    }

    pub(crate) fn borrow(&self, target: &WeakLinkTarget<'a>) -> Option<Ref<'_, T>> {
        Ref::filter_map(self.inner.targets.borrow(), |t| {
            t.get(&WeakPtrHash(Rc::downgrade(&target.inner)))
        })
        .ok()
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn drain(&self) -> Vec<T> {
        self.inner
            .targets
            .borrow_mut()
            .drain()
            .map(|(_, v)| v)
            .collect()
    }
}

struct WeakLinkInner<'a, T> {
    targets: RefCell<HashMap<WeakPtrHash<WeakLinkTargetInner<'a>>, T>>,
}

trait WeakLinkFrom<'a> {
    fn remove(&self, target: &WeakLinkTarget<'a>);
}

impl<'a, T> WeakLinkFrom<'a> for WeakLinkInner<'a, T> {
    fn remove(&self, target: &WeakLinkTarget<'a>) {
        self.targets
            .borrow_mut()
            .remove(&WeakPtrHash(Rc::downgrade(&target.inner)));
    }
}

impl<T> Drop for WeakLink<'_, T> {
    fn drop(&mut self) {
        for target in self.inner.targets.borrow().keys() {
            if let Some(target) = target.upgrade() {
                target.links.borrow_mut().remove(&WeakPtrHash(
                    Rc::downgrade(&self.inner) as Weak<dyn WeakLinkFrom>
                ));
            }
        }
    }
}

pub(crate) struct WeakLinkTarget<'a> {
    inner: Rc<WeakLinkTargetInner<'a>>,
}

impl WeakLinkTarget<'_> {
    pub(crate) fn new() -> Self {
        Self {
            inner: Rc::new(WeakLinkTargetInner {
                links: RefCell::new(HashSet::new()),
            }),
        }
    }
}

struct WeakLinkTargetInner<'a> {
    links: RefCell<HashSet<WeakPtrHash<dyn WeakLinkFrom<'a> + 'a>>>,
}

impl Drop for WeakLinkTarget<'_> {
    fn drop(&mut self) {
        for link in self.inner.links.borrow().iter() {
            if let Some(link) = link.upgrade() {
                link.remove(self);
            }
        }
    }
}
