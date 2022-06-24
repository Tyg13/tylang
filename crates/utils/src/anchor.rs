#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct Anchor {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}
