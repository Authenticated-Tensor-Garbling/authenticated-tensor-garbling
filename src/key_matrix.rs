use std::fmt::Display;
use std::ops::{BitXor, BitXorAssign, Index, IndexMut};

use crate::keys::Key;
use crate::block::Block;
use crate::delta::Delta;

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


/// Immutable view into a matrix with support for subviews and transposition.
/// 
/// This structure provides a safe way to create multiple views of the same underlying data
/// without violating Rust's borrow checker rules. Each view contains metadata about how to
/// interpret the data rather than holding separate references to the data.
pub struct MatrixViewRef<'a, T> {
    data: &'a [T],          // The actual data (immutable reference)
    total_rows: usize,      // Total rows in the original matrix
    total_cols: usize,      // Total columns in the original matrix
    view_start: usize,      // Starting index of this view
    view_rows: usize,       // Rows in this view
    view_cols: usize,       // Columns in this view
    transpose: bool,        // Whether this view is transposed
}

/// Mutable view into a matrix with support for subviews and transposition.
/// 
/// This structure provides a safe way to create multiple views of the same underlying data
/// without violating Rust's borrow checker rules. The `with_subrows` method uses closures
/// to ensure that only one mutable view exists at a time.
pub struct MatrixViewMut<'a, T> {
    data: &'a mut [T],      // The actual data (mutable reference)
    total_rows: usize,      // Total rows in the original matrix
    total_cols: usize,      // Total columns in the original matrix
    view_start: usize,      // Starting index of this view
    view_rows: usize,       // Rows in this view
    view_cols: usize,       // Columns in this view
    transpose: bool,        // Whether this view is transposed
}

impl<'a, T> MatrixViewRef<'a, T> {
    pub fn new(data: &'a [T], total_rows: usize, total_cols: usize) -> Self {
        Self {
            data,
            total_rows,
            total_cols,
            view_start: 0,
            view_rows: total_rows,
            view_cols: total_cols,
            transpose: false,
        }
    }

    pub fn transpose(&self) -> Self {
        Self {
            data: self.data,
            total_rows: self.total_rows,
            total_cols: self.total_cols,
            view_start: self.view_start,
            view_rows: self.view_cols,
            view_cols: self.view_rows,
            transpose: !self.transpose,
        }
    }

    pub fn shift(&self, row_offset: usize, col_offset: usize) -> Self {
        let new_start = self.view_start + col_offset * self.total_rows + row_offset; // Column-major layout
        Self {
            data: self.data,
            total_rows: self.total_rows,
            total_cols: self.total_cols,
            view_start: new_start,
            view_rows: self.view_rows,
            view_cols: self.view_cols,
            transpose: self.transpose,
        }
    }

    pub fn resize(&self, rows: usize, cols: usize) -> Self {
        Self {
            data: self.data,
            total_rows: self.total_rows,
            total_cols: self.total_cols,
            view_start: self.view_start,
            view_rows: rows,
            view_cols: cols,
            transpose: self.transpose,
        }
    }

    pub fn with_subrows<F, R>(&self, offset: usize, rows: usize, f: F) -> R
    where F: FnOnce(&MatrixViewRef<T>) -> R {
        let new_start = self.view_start + offset; // For column-major, row offset is just added directly
        let subview = MatrixViewRef {
            data: self.data,
            total_rows: self.total_rows,
            total_cols: self.total_cols,
            view_start: new_start,
            view_rows: rows,
            view_cols: self.view_cols,
            transpose: self.transpose,
        };
        f(&subview)
    }

    #[inline]
    pub fn rows(&self) -> usize { 
        if self.transpose { self.view_cols } else { self.view_rows }
    }
    #[inline]
    pub fn cols(&self) -> usize { 
        if self.transpose { self.view_rows } else { self.view_cols }
    }
    #[inline]
    pub fn len(&self) -> usize { 
        assert!(self.view_cols == 1, "Length is only defined for column vectors"); 
        self.view_rows 
    }
}

impl<'a, T> MatrixViewMut<'a, T> {
    pub fn new(data: &'a mut [T], total_rows: usize, total_cols: usize) -> Self {
        Self {
            data,
            total_rows,
            total_cols,
            view_start: 0,
            view_rows: total_rows,
            view_cols: total_cols,
            transpose: false,
        }
    }

    pub fn transpose(self) -> Self {
        Self {
            transpose: !self.transpose,
            ..self
        }
    }

    pub fn shift(&mut self, row_offset: usize, col_offset: usize) {
        self.view_start += col_offset * self.total_rows + row_offset; // Column-major layout
    }

    pub fn resize(&mut self, rows: usize, cols: usize) {
        self.view_rows = rows;
        self.view_cols = cols;
    }

    pub fn with_subrows<F, R>(&mut self, offset: usize, rows: usize, f: F) -> R
    where F: FnOnce(&mut MatrixViewMut<T>) -> R {
        let new_start = self.view_start + offset; // For column-major, row offset is just added directly
        let mut subview = MatrixViewMut {
            data: self.data,
            total_rows: self.total_rows,
            total_cols: self.total_cols,
            view_start: new_start,
            view_rows: rows,
            view_cols: self.view_cols,
            transpose: self.transpose,
        };
        f(&mut subview)
    }

    #[inline]
    pub fn rows(&self) -> usize { if self.transpose { self.view_cols } else { self.view_rows }}
    #[inline]
    pub fn cols(&self) -> usize { if self.transpose { self.view_rows } else { self.view_cols } }
    #[inline]
    pub fn len(&self) -> usize { 
        assert!(self.view_cols == 1, "Length is only defined for column vectors"); 
        self.view_rows 
    }
}

impl<'a, T> Index<(usize, usize)> for MatrixViewRef<'a, T> {
    type Output = T;
    
    fn index(&self, index: (usize, usize)) -> &Self::Output {
        let (row, col) = if self.transpose { (index.1, index.0) } else { (index.0, index.1) };
        let idx = self.view_start + col * self.total_rows + row; // Column-major layout
        &self.data[idx]
    }
}

impl<'a, T> Index<usize> for MatrixViewRef<'a, T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(self.view_cols == 1, "Vector indexing only works for column vectors");
        let idx = self.view_start + index;
        &self.data[idx]
    }
}

// Indexing for mutable views (read-write)
impl<'a, T> Index<(usize, usize)> for MatrixViewMut<'a, T> {
    type Output = T;

    fn index(&self, index: (usize, usize)) -> &Self::Output {
        let (row, col) = if self.transpose { (index.1, index.0) } else { (index.0, index.1) };
        let idx = self.view_start + col * self.total_rows + row; // Column-major layout
        &self.data[idx]
    }
}

impl<'a, T> IndexMut<(usize, usize)> for MatrixViewMut<'a, T> {
    fn index_mut(&mut self, index: (usize, usize)) -> &mut Self::Output {
        let (row, col) = if self.transpose { (index.1, index.0) } else { (index.0, index.1) };
        let idx = self.view_start + col * self.total_rows + row; // Column-major layout
        &mut self.data[idx]
    }
}

impl<'a, T> Index<usize> for MatrixViewMut<'a, T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(self.view_cols == 1, "Vector indexing only works for column vectors");
        let idx = self.view_start + index;
        &self.data[idx]
    }
}

impl<'a, T> IndexMut<usize> for MatrixViewMut<'a, T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        debug_assert!(self.view_cols == 1, "Vector indexing only works for column vectors");
        let idx = self.view_start + index;
        &mut self.data[idx]
    }
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

    pub fn random_zeros(rows: usize, cols: usize) -> Self {
        let mut elements = Vec::<T>::new();

        for _ in 0..(rows*cols) {
            let mut bytes = rand::random::<[u8; 16]>();
            bytes[0] &= 0xFE; // Clear last bit of last byte
            let label = T::from(bytes);
            elements.push(label);
        }

        Self {
            rows,
            cols,
            elements,
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
        MatrixViewRef::new(&self.elements[..], self.rows, self.cols)
    }

    pub fn as_view_mut(&mut self) -> MatrixViewMut<T> {
        MatrixViewMut::new(&mut self.elements[..], self.rows, self.cols)
    }

    pub fn with_subrows_mut<F, R>(&mut self, offset: usize, rows: usize, f: F) -> R
    where F: FnOnce(&mut MatrixViewMut<T>) -> R {
        let mut view = MatrixViewMut::new(&mut self.elements[..], self.rows, self.cols);
        view.with_subrows(offset, rows, f)
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

impl TypedMatrix<Block> {
    pub fn color_cross_product(&self, other: &Self, delta: Delta) -> Self {
        assert!(self.cols == 1 && other.cols == 1, "Color cross product only works for column vectors");

        let mut out = Self::new(self.rows, other.rows);
        for i in 0..self.rows {
            for j in 0..other.rows {
                out.elements[self.flat_index(i, j)] = if self.elements[i].lsb() & other.elements[j].lsb() {*delta.as_block()} else {Block::ZERO};
            }
        }
        out
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
impl<T: MatrixElement + BitXor<Output = T> + Copy> BitXor<TypedMatrix<T>> for TypedMatrix<T> {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {

        if self.rows != rhs.rows || self.cols != rhs.cols {
            panic!("Matrix dimensions must match");
        }

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

impl<'a, 'b, T: MatrixElement + BitXor<Output = T> + Copy> BitXor<&'b TypedMatrix<T>> for &'a TypedMatrix<T> {
    type Output = TypedMatrix<T>;

    fn bitxor(self, rhs: &'b TypedMatrix<T>) -> Self::Output {

        if self.rows != rhs.rows || self.cols != rhs.cols {
            panic!("Matrix dimensions must match");
        }

        let mut out = TypedMatrix {
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

impl<'a, 'b, 'c, T: MatrixElement + BitXor<Output = T> + Copy> BitXor<&'b MatrixViewRef<'c, T>> for &'a TypedMatrix<T> {
    type Output = TypedMatrix<T>;

    fn bitxor(self, rhs: &'b MatrixViewRef<T>) -> Self::Output {
        let mut out = TypedMatrix {
            rows: self.rows,
            cols: self.cols,
            elements: Vec::with_capacity(self.rows * self.cols),
        };
        for i in 0..self.rows {
            for j in 0..self.cols {
                let idx = self.flat_index(i, j);
                out.elements.push(self.elements[idx] ^ rhs[(i, j)]);
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

impl BlockMatrix {
    pub fn get_clear_value(&self) -> usize {
        // Endianness note (little-endian vectors):
        // We interpret index 0 as the least-significant bit (LSB) position of the vector,
        // and index rows-1 as the most-significant bit (MSB). The fold below iterates in
        // reverse to build the integer from MSB→LSB. If you change the vector endianness,
        // update this to match the new convention.
        let view = self.as_view();
        (0..view.rows()).rev().fold(0, |acc, i| {
            (acc << 1) | view[i].lsb() as usize
        })
    }
}

impl KeyMatrix {
    pub fn get_clear_value(&self) -> usize {
        let view = self.as_view();
        (0..view.rows()).rev().fold(0, |acc, i| {
            (acc << 1) | view[i].as_block().lsb() as usize
        })
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
