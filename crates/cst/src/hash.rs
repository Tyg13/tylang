use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub fn hash(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}
