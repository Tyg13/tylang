mod anchor;
mod span;
pub use anchor::*;
pub use span::*;

pub mod folding_set;
pub mod intern_map;
pub mod lru_cache;
pub mod order_map;
pub mod sparse_matrix;
pub mod vec_graph;

pub mod source_utils;
pub use source_utils::*;

pub mod string_utils;
pub use string_utils::*;

pub mod ap;

pub fn join<T>(
    out: &mut dyn std::io::Write,
    sep: &str,
    ts: impl Iterator<Item = T>,
    mut each: impl FnMut(&mut dyn std::io::Write, T) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let mut first = true;
    for t in ts {
        if !first {
            write!(out, "{sep}")?;
        }
        first = false;
        each(out, t)?;
    }
    Ok(())
}

#[macro_export]
macro_rules! newtype_idx {
    ($name:ident, $inner_ty:ty) => {
        #[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
        pub struct $name(pub(crate) $inner_ty);

        impl From<usize> for $name {
            fn from(v: usize) -> $name {
                $name(v as $inner_ty)
            }
        }

        impl From<$inner_ty> for $name {
            fn from(v: $inner_ty) -> $name {
                $name(v)
            }
        }

        impl $name {
            pub fn as_idx(self) -> usize {
                self.0 as usize
            }
        }
    };
    ($name:ident) => {
        newtype_idx!($name, u64);
    };
}
