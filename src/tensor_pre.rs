use crate::{aes::FixedKeyAes, block::Block, delta::Delta, matrix::{BlockMatrix}};

use rand::Rng;

pub struct TensorProductPreGen {
    pub cipher: &'static FixedKeyAes,
    pub chunking_factor: usize,
    pub n: usize,
    pub m: usize,
    pub delta: Delta,
    pub x: BlockMatrix,
    pub y: BlockMatrix,
    pub alpha: BlockMatrix,
    pub beta: BlockMatrix,
}

impl TensorProductPreGen {
    pub fn new(cipher: &'static FixedKeyAes, chunking_factor: usize, n: usize, m: usize, delta: Delta, x: BlockMatrix, y: BlockMatrix, alpha: BlockMatrix, beta: BlockMatrix) -> Self {
        assert!(x.rows() == n);
        assert!(x.cols() == 1);
        assert!(y.rows() == m);
        assert!(y.cols() == 1);
        assert!(alpha.rows() == n);
        assert!(alpha.cols() == 1);
        assert!(beta.rows() == m);
        assert!(beta.cols() == 1);

        Self { cipher, chunking_factor, n, m, delta, x, y, alpha, beta }
    }
}

pub struct TensorProductPreEval {
    pub cipher: &'static FixedKeyAes,
    pub chunking_factor: usize,
    pub n: usize,
    pub m: usize,
    pub x: BlockMatrix,
    pub y: BlockMatrix,
}

impl TensorProductPreEval {
    pub fn new(cipher: &'static FixedKeyAes, chunking_factor: usize, n: usize, m: usize, x: BlockMatrix, y: BlockMatrix) -> Self {
        assert!(x.rows() == n);
        assert!(x.cols() == 1);
        assert!(y.rows() == m);
        assert!(y.cols() == 1);

        Self { cipher, chunking_factor, n, m, x, y }
    }
}

pub fn get_gen_eval_vecs(delta: Delta, n: usize, clear_x: usize) -> (crate::matrix::TypedMatrix<Block>, crate::matrix::TypedMatrix<Block>) {
    let gen_x = BlockMatrix::random_zeros(n, 1);
    debug_assert!((0..n).all(|i| gen_x[i].lsb() == false), "gen_x LSBs must be 0");
    let mut eval_x = BlockMatrix::constant(n, 1, Block::default());
    
    // Debug: Print secret sharing details
    println!("=== SECRET SHARING DEBUG ===");
    println!("clear_x: {} (binary: {:b})", clear_x, clear_x);
    println!("gen_x LSBs: ", );
    for i in 0..n {
        print!("{}", gen_x[i].lsb() as u8);
    }
    println!();
    
    for i in 0..n {
        let bit = (clear_x >> i) & 1;
        println!("bit {}: clear_x[{}] = {}, gen_x[{}] = {}", i, i, bit, i, gen_x[i].lsb() as u8);
        eval_x[i] = if bit == 0 {
            gen_x[i]
        } else {
            gen_x[i] ^ delta
        };
        println!("  eval_x[{}] = {}", i, eval_x[i].lsb() as u8);
    }
    
    println!("eval_x LSBs (bit order 0,1,2): ", );
    for i in 0..n {
        print!("{}", eval_x[i].lsb() as u8);
    }
    println!();
    println!("eval_x LSBs (MSB to LSB): ", );
    for i in (0..n).rev() {
        print!("{}", eval_x[i].lsb() as u8);
    }
    println!();
    println!("============================");
    
    (gen_x, eval_x)
}

pub fn gen_masks(n: usize, m: usize, delta: &Delta) -> (BlockMatrix, BlockMatrix) {
    if n == 0 || m == 0{
        panic!("n and m must be greater than 0");
    }

    let mut alpha: BlockMatrix = BlockMatrix::new(n, 1);
    let mut beta: BlockMatrix = BlockMatrix::new(m, 1);

    let mut rng = rand::rng();
    for i in 0..n {
        let a_bit  = rng.random_bool(0.5);
        if a_bit {alpha[i] = *delta.as_block();} else {alpha[i] = Block::default();}
    }

    for i in 0..m{
        let b_bit = rng.random_bool(0.5);
        if b_bit {beta[i] = *delta.as_block();} else {beta[i] = Block::default();}
    }

    (alpha, beta)
}

pub fn setup_protocol(cipher: &'static FixedKeyAes, chunking_factor: usize, n: usize, m: usize, delta: Delta, clear_x: usize, clear_y: usize) -> (TensorProductPreGen, TensorProductPreEval) {
    
    let (gen_x, eval_x) = get_gen_eval_vecs(delta, n, clear_x);
    let (gen_y, eval_y) = get_gen_eval_vecs(delta, m, clear_y);

    let (alpha, beta) = gen_masks(n, m, &delta);
    let blinded_x = &eval_x ^ &alpha;
    let blinded_y = &eval_y ^ &beta;

    println!("=== SETUP MASKING DEBUG ===");
    println!("eval_y clear value: {}", eval_y.get_clear_value());
    println!("beta clear value: {}", beta.get_clear_value());
    println!("blinded_y clear value: {}", blinded_y.get_clear_value());
    
    // Test BitXor operation directly
    let test_result = &eval_y ^ &beta;
    println!("Direct test: eval_y ^ beta = {}", test_result.get_clear_value());
    println!("Expected: 4 ^ 1 = 5");
    println!("===========================");

    let pre_gen = TensorProductPreGen::new(cipher, chunking_factor, n, m, delta, gen_x, gen_y, alpha, beta);
    let pre_eval = TensorProductPreEval::new(cipher, chunking_factor, n, m, blinded_x, blinded_y);
    (pre_gen, pre_eval)
}