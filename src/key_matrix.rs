use std::fmt::Display;
use std::ops::{BitXor, BitXorAssign, Index, IndexMut};

use crate::keys::Key;
use crate::block::Block;

/// Trait for types that can be used in TypedMatrix
pub trait MatrixElement: 
    From<[u8; 16]> +
    Default + 
    Clone + 
    Copy + 
    PartialEq + 
    std::fmt::Debug + 
    Send + 
    Sync + 
    'static {}

impl MatrixElement for Key {}
impl MatrixElement for Block {}

#[inline]
fn shift_array(x: (usize, usize), y: (usize, usize)) -> (usize, usize) {
    (x.0 + y.0, x.1 + y.1)
}

pub struct MatrixView<T> {
    original_rows: usize,   // number of rows in the original matrix
    size: (usize, usize),   // (rows, cols)
    t: bool,                // transpose flag
    s: (usize, usize),      // shift position
    vals: T,            // reference to the underlying matrix
}

pub type MatrixViewRef<'a, T> = MatrixView<&'a [T]>;
pub type MatrixViewMut<'a, T> = MatrixView<&'a mut [T]>;

impl<'a, T> MatrixViewRef<'a, T> {
    pub fn transpose(&self) -> Self {
        Self {
            original_rows: self.original_rows,
            size: (self.size.1, self.size.0),
            t: !self.t,
            s: (self.s.1, self.s.0),
            vals: self.vals,
        }
    }

    pub fn shift(&self, s: (usize, usize)) -> Self {
        let new_s = shift_array(self.s, if self.t {(s.1, s.0)} else {s});
        Self { original_rows: self.original_rows, size: self.size, t: self.t, s: new_s, vals: self.vals}
    }

    pub fn resize(&self, n: usize, m: usize) -> Self {
        let new_size = (n, m);
        Self { original_rows: self.original_rows, size: new_size, t: self.t, s: self.s, vals: self.vals }
    }
}

impl<'a, T> Index<(usize, usize)> for MatrixViewRef<'a, T> {
    type Output = T;
    
    fn index(&self, index: (usize, usize)) -> &Self::Output {
        let (i_, j_) = if self.t { (index.1, index.0) } else { (index.0, index.1) };
        let (i_, j_) = shift_array((i_, j_), self.s);
        &self.vals[j_*self.original_rows + i_]
    }
}

impl<'a, T> Index<usize> for MatrixViewRef<'a, T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(self.cols() == 1, "Vector indexing only works for column vectors");
        &self.vals[index]
    }
}

// Indexing for mutable views (read-write)
impl<'a, T> Index<(usize, usize)> for MatrixViewMut<'a, T> {
    type Output = T;

    fn index(&self, index: (usize, usize)) -> &Self::Output {
        let (i_, j_) = if self.t { (index.1, index.0) } else { (index.0, index.1) };
        let (i_, j_) = shift_array((i_, j_), self.s);
        &self.vals[j_*self.original_rows + i_]
    }
}

impl<'a, T> IndexMut<(usize, usize)> for MatrixViewMut<'a, T> {
    fn index_mut(&mut self, index: (usize, usize)) -> &mut Self::Output {
        let (i_, j_) = if self.t { (index.1, index.0) } else { (index.0, index.1) };
        let (i_, j_) = shift_array((i_, j_), self.s);
        &mut self.vals[j_*self.original_rows + i_]
    }
}

impl<'a, T> Index<usize> for MatrixViewMut<'a, T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(self.cols() == 1, "Vector indexing only works for column vectors");
        &self.vals[index]
    }
}

impl<'a, T> IndexMut<usize> for MatrixViewMut<'a, T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        debug_assert!(self.cols() == 1, "Vector indexing only works for column vectors");
        &mut self.vals[index]
    }
}

impl<T> MatrixView<T> {
    pub fn new(original_rows: usize, size: (usize, usize), t: bool, s: (usize, usize), vals: T) -> Self {
        Self {
            original_rows,
            size,
            t,
            s,
            vals,
        }
    }

    pub fn rows(&self) -> usize { self.size.0 }
    pub fn cols(&self) -> usize { self.size.1 }
}

#[derive(Debug, Clone)]
pub struct TypedMatrix<T: MatrixElement> {
    rows: usize,
    cols: usize,
    elements: Vec<T>,
}

// Type alias for backward compatibility
pub type KeyMatrix = TypedMatrix<Key>;
pub type BlockMatrix = TypedMatrix<Block>;

impl<T: MatrixElement> TypedMatrix<T> {

    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            elements: vec![T::default(); rows * cols],
        }
    }

    pub fn column_vector(rows: usize) -> Self {
        Self {
            rows,
            cols: 1,
            elements: vec![T::default(); rows],
        }
    }

    pub fn random(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            elements: (0..rows * cols).map(|_| T::from(rand::random::<[u8; 16]>())).collect(),
        }
    }

    pub fn constant(rows: usize, cols: usize, val: T) -> Self {
        Self {
            rows,
            cols,
            elements: vec![val; rows * cols],
        }
    }

    pub fn as_view(&self) -> MatrixViewRef<T> {
        MatrixViewRef::new(self.rows, (self.rows, self.cols), false, (0, 0), &self.elements[..])
    }

    pub fn as_view_mut(&mut self) -> MatrixViewMut<T> {
        MatrixViewMut::new(self.rows, (self.rows, self.cols), false, (0, 0), &mut self.elements[..])
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    // helper function to convert 2D index to 1D index
    #[inline]
    fn flat_index(&self, i: usize, j: usize) -> usize {
        j*self.rows + i
    }
}

// Vector access
impl<T: MatrixElement> Index<usize> for TypedMatrix<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(self.cols == 1, "Vector indexing only works for column vectors");
        &self.elements[index]
    }
}

impl<T: MatrixElement> IndexMut<usize> for TypedMatrix<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        debug_assert!(self.cols == 1, "Vector indexing only works for column vectors");
        &mut self.elements[index]
    }
}

// Matrix access
impl<T: MatrixElement> Index<(usize, usize)> for TypedMatrix<T> {
    type Output = T;

    fn index(&self, index: (usize, usize)) -> &Self::Output {
        &self.elements[self.flat_index(index.0, index.1)]
    }
}

// Matrix access
impl<T: MatrixElement> IndexMut<(usize, usize)> for TypedMatrix<T> {

    fn index_mut(&mut self, index: (usize, usize)) -> &mut Self::Output {
        let idx = self.flat_index(index.0, index.1);
        &mut self.elements[idx]
    }
}

// BitXor implementation for TypedMatrix
impl<T: MatrixElement + BitXor<Output = T> + Copy> BitXor for TypedMatrix<T> {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        let mut out = Self {
            rows: self.rows,
            cols: self.cols,
            elements: Vec::with_capacity(self.rows * self.cols),
        };

        for i in 0..self.rows {
            for j in 0..self.cols {
                let idx = self.flat_index(i, j);
                out.elements.push(self.elements[idx] ^ rhs.elements[idx]);
            }
        }
        out
    }
}

// BitXorAssign implementation for TypedMatrix
impl<T: MatrixElement + BitXorAssign + Copy> BitXorAssign for TypedMatrix<T> {
    fn bitxor_assign(&mut self, rhs: Self) {
        self.elements.iter_mut().zip(rhs.elements.iter()).for_each(|(a, b)| {
            *a ^= *b;
        });
    }
}

impl<T: MatrixElement + Display> Display for TypedMatrix<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for i in 0..self.rows {
            for j in 0..self.cols {
                write!(f, "{} ", self.elements[self.flat_index(i, j)])?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let matrix = KeyMatrix::new(3, 4);
        assert_eq!(matrix.rows(), 3);
        assert_eq!(matrix.cols(), 4);
        assert_eq!(matrix.elements.len(), 12);
    }

    #[test]
    fn test_column_vector() {
        let vector = KeyMatrix::column_vector(5);
        assert_eq!(vector.rows(), 5);
        assert_eq!(vector.cols(), 1);
        assert_eq!(vector.elements.len(), 5);
    }

    #[test]
    fn test_flat_index() {
        let matrix = KeyMatrix::new(3, 4);
        // Test that flat_index correctly converts 2D indices to 1D using column-major indexing
        // For a 3x4 matrix, the layout is:
        // [0 3 6 9]
        // [1 4 7 10]
        // [2 5 8 11]
        assert_eq!(matrix.flat_index(0, 0), 0);  // row 0, col 0 -> index 0
        assert_eq!(matrix.flat_index(1, 0), 1);  // row 1, col 0 -> index 1
        assert_eq!(matrix.flat_index(2, 0), 2);  // row 2, col 0 -> index 2
        assert_eq!(matrix.flat_index(0, 1), 3);  // row 0, col 1 -> index 3
        assert_eq!(matrix.flat_index(1, 1), 4);  // row 1, col 1 -> index 4
        assert_eq!(matrix.flat_index(2, 1), 5);  // row 2, col 1 -> index 5
        assert_eq!(matrix.flat_index(0, 2), 6);  // row 0, col 2 -> index 6
        assert_eq!(matrix.flat_index(1, 2), 7);  // row 1, col 2 -> index 7
        assert_eq!(matrix.flat_index(2, 2), 8);  // row 2, col 2 -> index 8
        assert_eq!(matrix.flat_index(0, 3), 9);  // row 0, col 3 -> index 9
        assert_eq!(matrix.flat_index(1, 3), 10); // row 1, col 3 -> index 10
        assert_eq!(matrix.flat_index(2, 3), 11); // row 2, col 3 -> index 11
    }

    #[test]
    fn test_vector_indexing() {
        let mut matrix = KeyMatrix::column_vector(3);
        let key1 = Key::from(Block::new([0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]));
        let key2 = Key::from(Block::new([0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]));
        
        matrix[0] = key1;
        matrix[1] = key2;
        
        assert_eq!(matrix[0], key1);
        assert_eq!(matrix[1], key2);
        assert_eq!(matrix[2], Key::default());
    }

    #[test]
    fn test_bitxor_assign() {
        let mut matrix1 = KeyMatrix::new(2, 2);
        let mut matrix2 = KeyMatrix::new(2, 2);
        
        let key1 = Key::from(Block::new([0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]));
        let key2 = Key::from(Block::new([0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]));
        
        matrix1[(0, 0)] = key1;
        matrix1[(0, 1)] = key1;
        matrix1[(1, 0)] = key1;
        matrix1[(1, 1)] = key1;
        
        matrix2[(0, 0)] = key2;
        matrix2[(0, 1)] = key2;
        matrix2[(1, 0)] = key2;
        matrix2[(1, 1)] = key2;
        
        matrix1 ^= matrix2;
        
        assert_eq!(matrix1[(0, 0)], Key::from(Block::new([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])));
    }

    #[test]
    fn test_matrix_indexing() {
        let mut matrix = KeyMatrix::new(2, 3);
        
        let key1 = Key::from(Block::new([0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]));
        let key2 = Key::from(Block::new([0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22]));
        
        matrix[(0, 1)] = key1;
        matrix[(1, 2)] = key2;
        
        assert_eq!(matrix[(0, 1)], key1);
        assert_eq!(matrix[(1, 2)], key2);
        assert_eq!(matrix[(0, 0)], Key::default());
    }

    #[test]
    fn test_bitxor() {
        let mut matrix1 = KeyMatrix::new(2, 2);
        let mut matrix2 = KeyMatrix::new(2, 2);
        
        let key1 = Key::from(Block::new([0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]));
        let key2 = Key::from(Block::new([0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]));
        
        matrix1[(0, 0)] = key1;
        matrix1[(0, 1)] = key1;
        matrix1[(1, 0)] = key1;
        matrix1[(1, 1)] = key1;
        
        matrix2[(0, 0)] = key2;
        matrix2[(0, 1)] = key2;
        matrix2[(1, 0)] = key2;
        matrix2[(1, 1)] = key2;
        
        matrix1 ^= matrix2;
        
        // XOR of AAAAAAAA and 55555555 should be FFFFFFFF
        assert_eq!(matrix1[(0, 0)], Key::from(Block::new([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])));
        assert_eq!(matrix1[(0, 1)], Key::from(Block::new([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])));
        assert_eq!(matrix1[(1, 0)], Key::from(Block::new([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])));
        assert_eq!(matrix1[(1, 1)], Key::from(Block::new([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])));
    }

    #[test]
    fn test_display() {
        let mut matrix = KeyMatrix::new(2, 2);
        let key1 = Key::from(Block::new([0x12, 0x34, 0x56, 0x78, 0x90, 0xAB, 0xCD, 0xEF, 0x12, 0x34, 0x56, 0x78, 0x90, 0xAB, 0xCD, 0xEF]));
        let key2 = Key::from(Block::new([0xFE, 0xDC, 0xBA, 0x09, 0x87, 0x65, 0x43, 0x21, 0xFE, 0xDC, 0xBA, 0x09, 0x87, 0x65, 0x43, 0x21]));
        
        matrix[(0, 0)] = key1;
        matrix[(0, 1)] = key2;
        matrix[(1, 0)] = key2;
        matrix[(1, 1)] = key1;
        
        let display_str = format!("{}", matrix);
        println!("{}", display_str);
        // The display shows the last byte of each block in hex format
        // For the given test data, we expect to see the last bytes
        assert!(display_str.contains("ef"));
        assert!(display_str.contains("21"));
        assert!(display_str.lines().count() == 2);
    }

    #[test]
    fn test_zero_matrix() {
        let matrix = KeyMatrix::new(3, 4);
        for i in 0..3 {
            for j in 0..4 {
                assert_eq!(matrix[(i, j)], Key::default());
            }
        }
    }

    #[test]
    fn test_matrix_dimensions() {
        let matrix = KeyMatrix::new(5, 7);
        assert_eq!(matrix.rows(), 5);
        assert_eq!(matrix.cols(), 7);
        assert_eq!(matrix.elements.len(), 35);
    }

    #[test]
    #[should_panic(expected = "index out of bounds")]
    fn test_index_out_of_bounds() {
        let matrix = KeyMatrix::new(2, 2);
        let _ = matrix[(2, 2)]; // This should panic
    }

    #[test]
    fn test_vector_operations() {
        let mut vector = KeyMatrix::column_vector(4);
        let keys = vec![
            Key::from(Block::new([0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11])),
            Key::from(Block::new([0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22])),
            Key::from(Block::new([0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33])),
            Key::from(Block::new([0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44])),
        ];
        
        for (i, key) in keys.iter().enumerate() {
            vector[i] = *key;
        }
        
        for (i, key) in keys.iter().enumerate() {
            assert_eq!(vector[i], *key);
        }
    }

    #[test]
    fn test_generic_matrix_functionality() {
        // Test TypedMatrix with Key type
        let mut key_matrix = TypedMatrix::<Key>::new(2, 2);
        let key1 = Key::from(Block::new([0xAA; 16]));
        let key2 = Key::from(Block::new([0x55; 16]));
        
        key_matrix[(0, 0)] = key1;
        key_matrix[(1, 1)] = key2;
        
        assert_eq!(key_matrix[(0, 0)], key1);
        assert_eq!(key_matrix[(1, 1)], key2);
        
        // Test TypedMatrix with Block type
        let mut block_matrix = TypedMatrix::<Block>::new(2, 2);
        let block1 = Block::new([0xBB; 16]);
        let block2 = Block::new([0x66; 16]);
        
        block_matrix[(0, 0)] = block1;
        block_matrix[(1, 1)] = block2;
        
        assert_eq!(block_matrix[(0, 0)], block1);
        assert_eq!(block_matrix[(1, 1)], block2);
        
        // Test XOR operations on both types
        let key_matrix2 = TypedMatrix::<Key>::constant(2, 2, key2);
        let result_key = key_matrix.clone() ^ key_matrix2;
        
        let block_matrix2 = TypedMatrix::<Block>::constant(2, 2, block2);
        let result_block = block_matrix.clone() ^ block_matrix2;
        
        // Verify XOR results
        assert_eq!(result_key[(0, 0)], key1 ^ key2);
        assert_eq!(result_block[(0, 0)], block1 ^ block2);
        
        // Test MatrixView functionality
        let key_view = key_matrix.as_view();
        let block_view = block_matrix.as_view();
        
        assert_eq!(key_view[(0, 0)], key1);
        assert_eq!(block_view[(0, 0)], block1);
    }
}
