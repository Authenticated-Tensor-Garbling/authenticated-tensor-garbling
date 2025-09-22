use crate::{aes::{FixedKeyAes, FIXED_KEY_AES}, delta::Delta, fpre::{AuthBitShare}, auth_tensor_fpre::TensorFpreGen, block::Block};

pub struct auth_tensor_gen {
    cipher: &'static FixedKeyAes,
    seed: u64,
    n: usize,
    m: usize,
    delta_a: Delta,
    alpha_labels: Vec<Block>,
    beta_labels: Vec<Block>,
    alpha_auth_bit_shares: Vec<AuthBitShare>,
    beta_auth_bit_shares: Vec<AuthBitShare>,
    correlated_auth_bit_shares: Vec<AuthBitShare>,
    gamma_auth_bit_shares: Vec<AuthBitShare>,
}

impl auth_tensor_gen {
    pub fn new(seed: u64, n: usize, m: usize) -> Self {
        Self {
            cipher: &(*FIXED_KEY_AES),
            seed,
            n,
            m,
            delta_a: Delta::random(&mut rand::rng()),
            alpha_labels: Vec::new(),
            beta_labels: Vec::new(),
            alpha_auth_bit_shares: Vec::new(),
            beta_auth_bit_shares: Vec::new(),
            correlated_auth_bit_shares: Vec::new(),
            gamma_auth_bit_shares: Vec::new(),
        }
    }

    pub fn new_from_fpre_gen(seed: u64, fpre_gen: TensorFpreGen) -> Self {
        Self {
            cipher: &(*FIXED_KEY_AES),
            seed,
            n: fpre_gen.n,
            m: fpre_gen.m,
            delta_a: fpre_gen.delta_a,
            alpha_labels: Vec::new(),
            beta_labels: Vec::new(),
            alpha_auth_bit_shares: fpre_gen.alpha_auth_bit_shares,
            beta_auth_bit_shares: fpre_gen.beta_auth_bit_shares,
            correlated_auth_bit_shares: fpre_gen.correlated_auth_bit_shares,
            gamma_auth_bit_shares: fpre_gen.gamma_auth_bit_shares,
        }
    }
}