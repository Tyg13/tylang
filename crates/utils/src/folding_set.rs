// finish(
//  - Folding keys are bytestrings of arbitrary length
//  => Select which bytes of data structure should be considered for folding
//
//  i.e. FnTy(TyID, [TyID]) -> Fold(TyID) + Fold([TyID])

use smallvec::SmallVec;
use std::collections::HashMap;

#[derive(Debug, Default, Clone, Hash, PartialEq, Eq)]
pub struct FoldKey(SmallVec<[u8; 8]>);

impl FoldKey {
    pub fn add<T: Foldable>(&mut self, val: &T) {
        val.fold(self);
    }

    pub fn add_all<T: Foldable>(&mut self, vals: impl IntoIterator<Item = T>) {
        for val in vals {
            val.fold(self);
        }
    }
}

#[derive(Debug)]
pub struct FoldID<T: ?Sized>(u64, std::marker::PhantomData<T>);

impl<T: ?Sized> std::fmt::Display for FoldID<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: ?Sized> Default for FoldID<T> {
    fn default() -> Self {
        Self(0, std::marker::PhantomData)
    }
}

impl<T: ?Sized> std::hash::Hash for FoldID<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl<T: ?Sized> PartialEq for FoldID<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: ?Sized> Eq for FoldID<T> {}

impl<T: ?Sized> Clone for FoldID<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

impl<T: ?Sized> Copy for FoldID<T> {}

impl<T: ?Sized> FoldID<T> {
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
impl Foldable for bool {
    fn fold(&self, key: &mut FoldKey) {
        key.0.push(*self as u8);
    }
}

impl Foldable for &str {
    fn fold(&self, key: &mut FoldKey) {
        key.0.extend_from_slice(self.as_bytes());
    }
}

impl Foldable for String {
    fn fold(&self, key: &mut FoldKey) {
        key.0.extend_from_slice(self.as_bytes());
    }
}

impl<T: Foldable> Foldable for &'_ [T] {
    fn fold(&self, key: &mut FoldKey) {
        for v in *self {
            v.fold(key);
        }
    }
}

impl<T: Foldable> Foldable for &'_ T {
    fn fold(&self, key: &mut FoldKey) {
        key.add(*self);
    }
}

impl<T: Foldable> Foldable for Vec<T> {
    fn fold(&self, key: &mut FoldKey) {
        self.as_slice().fold(key)
    }
}

impl<T: Foldable> Foldable for (T,) {
    fn fold(&self, key: &mut FoldKey) {
        self.0.fold(key);
    }
}
impl<T: Foldable> Foldable for (T, T) {
    fn fold(&self, key: &mut FoldKey) {
        self.0.fold(key);
        self.1.fold(key);
    }
}
impl<T: Foldable> Foldable for (T, T, T) {
    fn fold(&self, key: &mut FoldKey) {
        self.0.fold(key);
        self.1.fold(key);
        self.2.fold(key);
    }
}

impl<T: ?Sized> Foldable for FoldID<T> {
    fn fold(&self, key: &mut FoldKey) {
        self.0.fold(key);
    }
}

impl Foldable for FoldKey {
    fn fold(&self, key: &mut FoldKey) {
        key.0.extend_from_slice(self.0.as_slice());
    }
}

#[derive(Debug, Clone)]
pub struct FoldingSet<T: Foldable> {
    data: Vec<T>,
    keys_to_ids: HashMap<FoldKey, FoldID<T>>,
}

impl<'a, T: Foldable> IntoIterator for &'a FoldingSet<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.iter()
    }
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

    pub fn insert(&mut self, key: FoldKey, v: T) -> FoldID<T> {
        let id = FoldID::new(self.data.len() + 1);
        self.data.push(v);
        self.keys_to_ids.insert(key, id);
        id
    }

    pub fn insert_and_then<R>(
        &mut self,
        key: FoldKey,
        v: T,
        f: impl FnOnce(&mut T, FoldID<T>) -> R,
    ) -> R {
        let id = self.insert(key, v);
        f(self.get_mut(&id).unwrap(), id)
    }

    pub fn fold_or_insert(&mut self, v: T) -> FoldID<T> {
        let key = v.fold_key();
        if let Some(id) = self.keys_to_ids.get(&key) {
            return *id;
        }
        self.insert(key, v)
    }

    pub fn try_get_from_key(&self, key: &FoldKey) -> Option<&T> {
        self.keys_to_ids.get(key).map(|id| self.get(id).unwrap())
    }

    pub fn id_from_key(&self, key: &FoldKey) -> Option<FoldID<T>> {
        self.keys_to_ids.get(key).copied()
    }

    pub fn id_from_val(&self, val: &T) -> Option<FoldID<T>> {
        self.id_from_key(&val.fold_key())
    }

    pub fn get(&self, id: &FoldID<T>) -> Option<&T> {
        debug_assert_ne!(id.0, 0);
        self.data.get(id.0 as usize - 1)
    }

    pub fn get_mut(&mut self, id: &FoldID<T>) -> Option<&mut T> {
        debug_assert_ne!(id.0, 0);
        self.data.get_mut(id.0 as usize - 1)
    }
}

#[macro_export]
macro_rules! impl_foldable {
    ($s:ident$(<$($life:lifetime),*>)?, $($field:ident : $field_ty:ty),*) => {
        impl$(<$($life),*>)? $s$(<$($life),*>)? {
            fn profile($($field : &$field_ty),*) -> $crate::folding_set::FoldKey {
                let mut key = $crate::folding_set::FoldKey::default();
                $(key.add($field);)*
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
