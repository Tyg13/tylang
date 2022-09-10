#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct ID(pub usize);
pub const NONE: ID = ID(usize::MAX);

impl Default for ID {
    fn default() -> Self {
        return NONE;
    }
}
