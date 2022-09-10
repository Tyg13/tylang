mod anchor;
mod span;
pub use anchor::*;
pub use span::*;

pub mod intern_map;
pub mod sparse_matrix;
pub mod vec_graph;

pub mod source_utils;
pub use source_utils::*;

pub mod string_utils;
pub use string_utils::*;

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
