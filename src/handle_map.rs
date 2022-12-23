use std::collections::{hash_map::Values, HashMap};

pub(crate) struct HandleMap<T> {
    next_id: usize,
    map: HashMap<usize, T>,
}

pub(crate) struct Handle {
    id: usize,
}

impl<T> HandleMap<T> {
    pub(crate) fn new() -> Self {
        Self {
            next_id: 0,
            map: HashMap::new(),
        }
    }

    pub(crate) fn insert(&mut self, value: T) -> Handle {
        let id = self.next_id;
        self.next_id += 1;
        self.map.insert(id, value);
        Handle { id }
    }

    pub(crate) fn remove(&mut self, handle: Handle) {
        self.map.remove(&handle.id);
    }

    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }
}

impl<'iter, T> IntoIterator for &'iter HandleMap<T> {
    type Item = &'iter T;
    type IntoIter = Values<'iter, usize, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.map.values()
    }
}
