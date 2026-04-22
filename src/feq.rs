//! Ideal F_eq functionality: element-wise BlockMatrix equality check.
//!
//! In the real protocol, parties send L_1 and L_2 to F_eq, which compares
//! them and returns 0 (abort) if they differ, 1 (continue) otherwise. This
//! in-process ideal version panics on mismatch, matching the protocol's
//! abort semantics (per CONTEXT.md D-04).
//!
//! TODO: Replace with a real equality-check protocol (e.g., commit-and-open
//!       hash) for production.

use crate::matrix::BlockMatrix;

/// Ideal F_eq check. Panics with `"F_eq abort: ..."` on any mismatched entry,
/// and with `"F_eq: row dimension mismatch"` or `"F_eq: column dimension mismatch"`
/// if the two inputs are not the same shape.
///
/// Iteration order matches the codebase's column-major convention:
/// outer loop over columns, inner loop over rows (see `src/matrix.rs`).
pub fn check(l1: &BlockMatrix, l2: &BlockMatrix) {
    assert_eq!(l1.rows(), l2.rows(), "F_eq: row dimension mismatch");
    assert_eq!(l1.cols(), l2.cols(), "F_eq: column dimension mismatch");
    for j in 0..l1.cols() {
        for i in 0..l1.rows() {
            if l1[(i, j)] != l2[(i, j)] {
                panic!(
                    "F_eq abort: consistency check failed — L_1 != L_2 at ({}, {})",
                    i, j
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::Block;

    #[test]
    fn test_check_equal_matrices_passes() {
        let mut a = BlockMatrix::new(3, 4);
        let mut b = BlockMatrix::new(3, 4);
        for j in 0..4 {
            for i in 0..3 {
                let v = Block::new([((i as u8) ^ (j as u8)); 16]);
                a[(i, j)] = v;
                b[(i, j)] = v;
            }
        }
        check(&a, &b); // must not panic
    }

    #[test]
    #[should_panic(expected = "F_eq abort")]
    fn test_check_differing_matrices_panics() {
        let a = BlockMatrix::new(2, 2);
        let mut b = BlockMatrix::new(2, 2);
        b[(0, 0)] = Block::new([1; 16]);
        check(&a, &b);
    }

    #[test]
    #[should_panic(expected = "F_eq: row dimension mismatch")]
    fn test_check_dimension_mismatch_panics() {
        let a = BlockMatrix::new(2, 2);
        let b = BlockMatrix::new(3, 2);
        check(&a, &b);
    }
}
