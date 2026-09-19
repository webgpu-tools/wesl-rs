use crate::FastIndexSet;

/// An append-only arena
#[derive(Clone)]
pub struct Arena<T> {
    items: Vec<T>,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T> Arena<T> {
    pub fn add(&mut self, item: T) -> Id<T> {
        let index = u64::try_from(self.items.len()).unwrap();
        self.items.push(item);
        Id::new_unchecked(index)
    }

    pub fn get(&self, id: Id<T>) -> Option<&T> {
        self.items.get(id.as_usize())
    }
}

impl<T> std::ops::Index<Id<T>> for Arena<T> {
    type Output = T;

    #[inline]
    fn index(&self, id: Id<T>) -> &Self::Output {
        &self.items[id.as_usize()]
    }
}

/// An append-only arena with unique elements.
/// Lookups are direct index lookups and require no hashing.
#[derive(Debug, Clone)]
pub struct UniqueArena<T> {
    items: FastIndexSet<T>,
}

impl<T> Default for UniqueArena<T> {
    fn default() -> Self {
        Self {
            items: FastIndexSet::default(),
        }
    }
}

impl<T: Eq + std::hash::Hash> UniqueArena<T> {
    pub fn add(&mut self, item: T) -> Id<T> {
        let (index, _) = self.items.insert_full(item);
        Id::new_unchecked(u64::try_from(index).unwrap())
    }

    pub fn get(&self, id: Id<T>) -> Option<&T> {
        self.items.get_index(id.as_usize())
    }
}

impl<T> std::ops::Index<Id<T>> for UniqueArena<T> {
    type Output = T;

    #[inline]
    fn index(&self, id: Id<T>) -> &Self::Output {
        &self.items[id.as_usize()]
    }
}

pub struct Id<T> {
    index: u64,
    _marker: std::marker::PhantomData<T>,
}
impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}
impl<T> Eq for Id<T> {}

impl<T> std::fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Id").field("index", &self.index).finish()
    }
}

impl<T> std::hash::Hash for Id<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

impl<T> Id<T> {
    pub fn new_unchecked(index: u64) -> Self {
        Self {
            index,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn as_u64(self) -> u64 {
        self.index
    }

    pub fn as_usize(self) -> usize {
        usize::try_from(self.index).unwrap()
    }
}
