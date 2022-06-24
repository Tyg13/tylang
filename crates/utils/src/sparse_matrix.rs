use std::collections::HashMap;

#[derive(Debug)]
pub struct SparseMatrix<T> {
    dimensions: (usize, usize),
    values: Vec<T>,
    indices: HashMap<usize, usize>,
}

impl<T> SparseMatrix<T> {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            dimensions: (width, height),
            values: Vec::new(),
            indices: HashMap::new(),
        }
    }

    pub fn insert(&mut self, (x, y): (usize, usize), v: T) -> bool {
        let index = x * self.dimensions.0 + y;
        if self.indices.get(&index).is_some() {
            return true;
        }
        let value_index = self.values.len();
        self.values.push(v);
        self.indices.insert(index, value_index);
        false
    }

    pub fn get(&self, (x, y): (usize, usize)) -> Option<&T> {
        let index = x * self.dimensions.0 + y;
        self.indices.get(&index).map(|value_index| {
            self.values
                .get(*value_index)
                .expect("internal matrix inconsistency")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_matrix() {
        let mut matrix = SparseMatrix::new(10, 10);
        matrix.insert((0, 9), 10);
        assert_eq!(matrix.get((0, 9)), Some(&10));
        assert_eq!(matrix.get((1, 1)), None);
    }
}
