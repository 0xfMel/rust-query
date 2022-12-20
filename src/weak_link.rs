use std::{
    cell::{Ref, RefCell},
    collections::{
        hash_map::{Entry as HashMapEntry, OccupiedEntry as HashMapOccupied},
        HashMap, HashSet,
    },
    rc::{Rc, Weak},
};

use crate::ptr_hash::HashWeakPtr;

/// Represents a link between two objects, links with a [`Target`]
/// Stores a ``T`` for each [`Target`] it is linked to
pub(crate) struct WeakLink<'link, T: 'link> {
    /// Inner state, wrapped in an [`Rc`] so each object that links can have an owned value
    inner: Rc<WeakLinkInner<'link, T>>,
}

impl<T> Clone for WeakLink<'_, T> {
    fn clone(&self) -> Self {
        WeakLink {
            inner: Rc::clone(&self.inner),
        }
    }
}

/// Replacement of [`std::collections::hash_map::Entry`] for our wrapper types
pub(crate) enum Entry<'entry, 'link, T> {
    /// There is no data stored about this link
    Vacant, /*(VacantEntry<'entry, 'link, T>)*/
    /// There is data stored about this link
    Occupied(OccupiedEntry<'entry, 'link, T>),
}

/* /// Wrapper around [`std::collections::hash_map::VacantEntry`]
pub(crate) struct VacantEntry<'entry, 'link, T> {
    /// Inner entry value from [`HashMap`]
    _entry: HashMapVacant<'entry, HashWeakPtr<TargetInner<'link>>, T>,
}*/

/// Wrapper around [`std::collections::hash_map::OccupiedEntry`]
pub(crate) struct OccupiedEntry<'entry, 'link, T> {
    /// Inner entry value from [`HashMap`]
    entry: HashMapOccupied<'entry, HashWeakPtr<TargetInner<'link>>, T>,
}

/* impl<'entry, T> VacantEntry<'entry, '_, T> {
    /// See [`std::collections::hash_map::VacantEntry::insert`]
    #[inline]
    pub(crate) fn insert(self, value: T) -> &'entry mut T {
        self.entry.insert(value)
    }
}*/

impl<T> OccupiedEntry<'_, '_, T> {
    /// See [`std::collections::hash_map::OccupiedEntry::get`]
    #[inline]
    pub(crate) fn get(&self) -> &T {
        self.entry.get()
    }

    /// See [`std::collections::hash_map::OccupiedEntry::get_mut`]
    #[inline]
    pub(crate) fn get_mut(&mut self) -> &mut T {
        self.entry.get_mut()
    }

    #[inline]
    pub(crate) fn remove(self) -> T {
        self.entry.remove()
    }
}

/*impl<'link, T: Default> WeakLink<'link, T> {
    pub(crate) fn with_or_default<R>(
        &self,
        target: &Target<'link>,
        f: impl FnOnce(&mut T) -> R,
    ) -> R {
        let mut targets = self.inner.targets.borrow_mut();
        let value = targets.entry(self.link(target)).or_insert_with(T::default);
        f(value)
    }
}*/

impl<'link, T> WeakLink<'link, T> {
    /// Creates a new [`WeakLink`]
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            inner: Rc::new(WeakLinkInner {
                targets: RefCell::new(HashMap::new()),
            }),
        }
    }

    fn link(&self, target: &Target<'link>) -> HashWeakPtr<TargetInner<'link>> {
        // Non-trivial
        #[allow(trivial_casts)]
        target.inner.links.borrow_mut().insert(HashWeakPtr(
            Rc::downgrade(&self.inner) as Weak<dyn WeakLinkFrom<'link>>
        ));

        HashWeakPtr(Rc::downgrade(&target.inner))
    }

    pub(crate) fn with_entry<R>(
        &self,
        target: &Target<'link>,
        f: impl FnOnce(Entry<'_, 'link, T>) -> R,
    ) -> R {
        let mut targets = self.inner.targets.borrow_mut();
        let entry = targets.entry(self.link(target));
        f(match entry {
            HashMapEntry::Occupied(o) => Entry::Occupied(OccupiedEntry { entry: o }),
            HashMapEntry::Vacant(_v) => Entry::Vacant, /*(VacantEntry { _entry: v })*/
        })
    }

    pub(crate) fn with_or_else<R>(
        &self,
        target: &Target<'link>,
        default: impl FnOnce() -> T,
        f: impl FnOnce(&mut T) -> R,
    ) -> R {
        let mut targets = self.inner.targets.borrow_mut();
        let value = targets.entry(self.link(target)).or_insert_with(default);
        f(value)
    }

    /// Borrows value from the interal [`RefCell`] of the [`WeakLink`] for the link between it and a [`Target`]
    /// Returns None if no link has been made between the two
    pub(crate) fn borrow(&self, target: &Target<'link>) -> Option<Ref<'_, T>> {
        Ref::filter_map(self.inner.targets.borrow(), |t| {
            t.get(&HashWeakPtr(Rc::downgrade(&target.inner)))
        })
        .ok()
    }
}

/// Internal state of a [`WeakLink`]
struct WeakLinkInner<'link, T> {
    /// [`HashMap`] of the link between this [`WeakLink`] and a target and its associated value
    targets: RefCell<HashMap<HashWeakPtr<TargetInner<'link>>, T>>,
}

/// Trait to allow for a [`Target`] to contain references to [`WeakLink`]s it is associated to
/// without needing to be generic over its associated value type
trait WeakLinkFrom<'link> {
    /// Remove a link for the given [`Target`] and drop its associated value
    fn remove(&self, target: &Target<'link>);
}

impl<'link, T> WeakLinkFrom<'link> for WeakLinkInner<'link, T> {
    fn remove(&self, target: &Target<'link>) {
        self.targets
            .borrow_mut()
            .remove(&HashWeakPtr(Rc::downgrade(&target.inner)));
    }
}

impl<'link, T> Drop for WeakLink<'link, T> {
    fn drop(&mut self) {
        for target in self.inner.targets.borrow().keys() {
            if let Some(target) = target.upgrade() {
                // Non-trivial
                #[allow(trivial_casts)]
                target.links.borrow_mut().remove(&HashWeakPtr(
                    Rc::downgrade(&self.inner) as Weak<dyn WeakLinkFrom<'link>>
                ));
            }
        }
    }
}

/// Represents the target of a [`WeakLink`], wrapper type around an [`Rc`] with the internal state
/// to allow each target to be an owned type
pub(crate) struct Target<'link> {
    /// Internal state
    inner: Rc<TargetInner<'link>>,
}

impl Target<'_> {
    /// Creates a new [`Target`]
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            inner: Rc::new(TargetInner {
                links: RefCell::new(HashSet::new()),
            }),
        }
    }
}

/// Internal state of a [`Target`]
struct TargetInner<'link> {
    /// A set of each [`WeakLink`] this [`Target`] is linked to, using the [`WeakLinkFrom`] trait
    links: RefCell<HashSet<HashWeakPtr<dyn WeakLinkFrom<'link> + 'link>>>,
}

impl Drop for Target<'_> {
    fn drop(&mut self) {
        for link in self.inner.links.borrow_mut().iter() {
            if let Some(link) = link.upgrade() {
                link.remove(self);
            }
        }
    }
}
