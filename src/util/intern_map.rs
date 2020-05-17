use bimap::BiHashMap;

#[macro_export]
macro_rules! declare_new_intern_id {
    ($name:ident) => {
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        pub struct $name(usize);

        impl $crate::util::intern_map::Id for $name {
            fn new(id: usize) -> Self {
                Self(id)
            }
        }
    };
}

pub trait Id {
    fn new(id: usize) -> Self;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InternMap<Key, Value>
where
    Key: Clone + Copy + Eq + Id + PartialEq + std::fmt::Debug + std::hash::Hash,
    Value: Clone + Eq + PartialEq + std::fmt::Debug + std::hash::Hash,
{
    inner: BiHashMap<Key, Value>,
}

impl<Key, Value> InternMap<Key, Value>
where
    Key: Clone + Copy + Eq + Id + PartialEq + std::fmt::Debug + std::hash::Hash,
    Value: Clone + Eq + PartialEq + std::fmt::Debug + std::hash::Hash,
{
    pub fn new() -> Self {
        Self {
            inner: BiHashMap::new(),
        }
    }

    pub fn add(&mut self, s: Value) -> Key {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;
        match self.inner.get_by_right(&s) {
            Some(id) => *id,
            None => {
                let mut hasher = DefaultHasher::new();
                s.hash(&mut hasher);
                let id = Key::new(hasher.finish() as usize);
                match self.inner.insert(id, s) {
                    bimap::Overwritten::Neither => {}
                    _ => panic!("Interned values should not be overwritten!"),
                }
                id
            }
        }
    }

    pub fn get(&self, id: &Key) -> Option<&Value> {
        self.inner.get_by_left(id)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }
}
