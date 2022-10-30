// Sketch:
//  - Folding keys are bytestrings of arbitrary length
//  => Select which bytes of data structure should be considered for folding
//
//  i.e. FnTy(TyID, [TyID]) -> Fold(TyID) + Fold([TyID])

use std::collections::HashMap;

#[derive(Debug, Default, Clone, Hash, PartialEq, Eq)]
pub struct FoldKey(Vec<u8>);

impl FoldKey {
    pub fn add<T: Foldable>(mut self, val: &T) -> Self {
        val.fold(&mut self);
        self
    }
}

#[derive(Debug)]
pub struct FoldID<T>(u64, std::marker::PhantomData<T>);

impl<T> std::hash::Hash for FoldID<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl<T> PartialEq for FoldID<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> Eq for FoldID<T> {}

impl<T> Clone for FoldID<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

impl<T> Copy for FoldID<T> {}

impl<T> FoldID<T> {
    fn new(v: usize) -> Self {
        Self(v as u64, std::marker::PhantomData)
    }
}

pub trait Foldable {
    fn fold(&self, key: &mut FoldKey);
    fn fold_key(&self) -> FoldKey {
        let mut key = FoldKey::default();
        self.fold(&mut key);
        key
    }
}

macro_rules! impl_primitive_fold {
    ($($t:ident),*) => {
        $(
            impl Foldable for $t {
                fn fold(&self, key: &mut FoldKey) {
                    key.0.extend_from_slice(&self.to_le_bytes());
                }
            }
        )*
    }
}
impl_primitive_fold!(u64, u32, u16, i64, i32, i16, usize, isize);

impl Foldable for u8 {
    fn fold(&self, key: &mut FoldKey) {
        key.0.push(*self);
    }
}
impl Foldable for i8 {
    fn fold(&self, key: &mut FoldKey) {
        key.0.push(*self as u8);
    }
}

impl Foldable for &str {
    fn fold(&self, key: &mut FoldKey) {
        key.0.extend_from_slice(self.as_bytes());
    }
}

impl<'a, T: Foldable> Foldable for &'a [T] {
    fn fold(&self, key: &mut FoldKey) {
        for v in *self {
            v.fold(key);
        }
    }
}

#[derive(Debug, Clone)]
pub struct FoldingSet<T: Foldable> {
    data: Vec<T>,
    keys_to_ids: HashMap<FoldKey, FoldID<T>>,
}

impl<T: Foldable> Default for FoldingSet<T> {
    fn default() -> Self {
        Self {
            data: Default::default(),
            keys_to_ids: Default::default(),
        }
    }
}

impl<T: Foldable> FoldingSet<T> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn fold_or_insert(&mut self, v: T) -> FoldID<T> {
        let key = v.fold_key();
        if let Some(id) = self.keys_to_ids.get(&key) {
            return *id;
        }
        let id = FoldID::new(self.data.len());
        self.data.push(v);
        self.keys_to_ids.insert(key, id);
        id
    }

    pub fn try_get_from_key(&self, key: &FoldKey) -> Option<&T> {
        self.keys_to_ids.get(key).map(|id| self.get(id).unwrap())
    }

    pub fn get(&self, id: &FoldID<T>) -> Option<&T> {
        self.data.get(id.0 as usize)
    }

    pub fn get_mut(&mut self, id: &FoldID<T>) -> Option<&mut T> {
        self.data.get_mut(id.0 as usize)
    }
}

#[macro_export]
macro_rules! impl_foldable {
    ($s:ident$(<$($life:lifetime),*>)?, $($field:ident : $field_ty:ty),*) => {
        impl$(<$($life),*>)? $s$(<$($life),*>)? {
            fn profile($($field : &$field_ty),*) -> $crate::folding_set::FoldKey {
                let mut key = $crate::folding_set::FoldKey::default();
                $(key = key.add($field);)*
                key
            }
        }
        impl$(<$($life),*>)? Foldable for $s$(<$($life),*>)? {
            fn fold(&self, key: &mut $crate::folding_set::FoldKey) {
                $(self.$field.fold(key);)*
            }
            fn fold_key(&self) -> $crate::folding_set::FoldKey {
                $s::profile($(&self.$field),*)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct Item {
        name: &'static str,
        fold: u64,
        nofold: u8,
    }
    impl_foldable!(Item, name: &'static str, fold: u64);

    #[test]
    fn can_fold_equal_value() {
        let mut set = FoldingSet::new();
        let a = set.fold_or_insert(Item {
            name: "Foo",
            fold: 1,
            nofold: 2,
        });
        let b = set.fold_or_insert(Item {
            name: "Foo",
            fold: 1,
            nofold: 10,
        });
        assert_eq!(a, b);
        assert_eq!(
            set.get(&a),
            Some(&Item {
                name: "Foo",
                fold: 1,
                nofold: 2
            })
        );
        assert_eq!(
            set.get(&b),
            Some(&Item {
                name: "Foo",
                fold: 1,
                nofold: 2
            })
        );
    }

    #[test]
    fn can_insert_inequal_value() {
        let mut set = FoldingSet::new();
        let a = set.fold_or_insert(Item {
            name: "Foo",
            fold: 1,
            nofold: 2,
        });
        let b = set.fold_or_insert(Item {
            name: "Foo",
            fold: 2,
            nofold: 2,
        });
        assert_ne!(a, b);
        assert_eq!(
            set.get(&a),
            Some(&Item {
                name: "Foo",
                fold: 1,
                nofold: 2
            })
        );
        assert_eq!(
            set.get(&b),
            Some(&Item {
                name: "Foo",
                fold: 2,
                nofold: 2
            })
        );
    }

    #[test]
    fn can_get_from_equal_key() {
        let mut set = FoldingSet::new();
        set.fold_or_insert(Item {
            name: "Bar",
            fold: 1,
            nofold: 2,
        });
        let key = Item::profile(&"Bar", &1u64);
        assert_eq!(
            set.try_get_from_key(&key),
            Some(&Item {
                name: "Bar",
                fold: 1,
                nofold: 2
            })
        );
    }

    #[test]
    fn cant_get_from_inequal_key() {
        let mut set = FoldingSet::new();
        set.fold_or_insert(Item {
            name: "Bar",
            fold: 1,
            nofold: 2,
        });
        let key = Item::profile(&"Baz", &1u64);
        assert_eq!(set.try_get_from_key(&key), None);
    }
}
