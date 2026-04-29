use crate::{
    block::Block,
    delta::Delta,
    sharing::InputSharing,
};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;

pub struct SemiHonestTensorPre {
    pub rng: ChaCha12Rng,
    pub n: usize,
    pub m: usize,
    pub chunking_factor: usize,
    pub delta: Delta,
    pub x_labels: Vec<InputSharing>,
    pub y_labels: Vec<InputSharing>,
    pub alpha_labels: Vec<InputSharing>,
    pub beta_labels: Vec<InputSharing>,
}

impl SemiHonestTensorPre {
    pub fn new(seed: u64, n: usize, m: usize, chunking_factor: usize) -> Self {
        Self {
            rng: ChaCha12Rng::seed_from_u64(seed),
            n,
            m,
            chunking_factor,
            delta: Delta::random_gb(&mut rand::rng()),
            x_labels: Vec::with_capacity(n),
            y_labels: Vec::with_capacity(m),
            alpha_labels: Vec::with_capacity(n),
            beta_labels: Vec::with_capacity(m),
        }
    }

    pub fn new_with_delta(seed: u64, n: usize, m: usize, chunking_factor: usize, delta: Delta) -> Self {
        Self {
            rng: ChaCha12Rng::seed_from_u64(seed),
            n,
            m,
            chunking_factor,
            delta,
            x_labels: Vec::with_capacity(n),
            y_labels: Vec::with_capacity(m),
            alpha_labels: Vec::with_capacity(n),
            beta_labels: Vec::with_capacity(m),
        }
    }

    pub fn gen_inputs(&mut self, x: usize, y: usize) {
        assert!(x < 1<<self.n);
        assert!(y < 1<<self.m);

        for i in 0..self.n {
            let x_bit = (x >> i) & 1 != 0;
            let mut gb_share = Block::random(&mut self.rng);
            gb_share.set_lsb(false);
            let ev_share = if x_bit { gb_share ^ self.delta.as_block() } else { gb_share };

            self.x_labels.push(InputSharing { gen_share: gb_share, eval_share: ev_share });

        }

        for j in 0..self.m {
            let y_bit = (y >> j) & 1 != 0;
            let mut gb_share = Block::random(&mut self.rng);
            gb_share.set_lsb(false);
            let ev_share = if y_bit { gb_share ^ self.delta.as_block() } else { gb_share };

            self.y_labels.push(InputSharing { gen_share: gb_share, eval_share: ev_share });
        }
    }

    pub fn gen_masks(&mut self) -> (usize, usize) {

        let mut alpha = 0;
        for i in 0..self.n {
            let alpha_bit = self.rng.random_bool(0.5);
            let gb_share = if alpha_bit { *self.delta.as_block() } else { Block::default() };
            let ev_share = Block::default();
            
            alpha |= (alpha_bit as usize) << i;

            self.alpha_labels.push(InputSharing { gen_share: gb_share, eval_share: ev_share });
        }

        let mut beta = 0;
        for j in 0..self.m {
            let beta_bit = self.rng.random_bool(0.5);
            let gb_share = if beta_bit { *self.delta.as_block() } else { Block::default() };
            let ev_share = Block::default();

            beta |= (beta_bit as usize) << j;

            self.beta_labels.push(InputSharing { gen_share: gb_share, eval_share: ev_share });
        }

        (alpha, beta)
    }

    pub fn mask_inputs(&mut self) -> (usize, usize) {

        assert!(self.x_labels.len() == self.alpha_labels.len());
        assert!(self.y_labels.len() == self.beta_labels.len());

        let mut masked_x = 0;
        for i in 0..self.n {
            masked_x |= (self.x_labels[i].shares_differ() as usize ^ self.alpha_labels[i].shares_differ() as usize) << i;

            self.x_labels[i] = InputSharing {
                gen_share: self.x_labels[i].gen_share ^ self.alpha_labels[i].eval_share,
                eval_share: self.x_labels[i].eval_share ^ self.alpha_labels[i].gen_share,
            }
        }

        let mut masked_y = 0;
        for j in 0..self.m {
            masked_y |= (self.y_labels[j].shares_differ() as usize ^ self.beta_labels[j].shares_differ() as usize) << j;

            self.y_labels[j] = InputSharing {
                gen_share: self.y_labels[j].gen_share ^ self.beta_labels[j].eval_share,
                eval_share: self.y_labels[j].eval_share ^ self.beta_labels[j].gen_share,
            }
        }

        (masked_x, masked_y)
    }

    pub fn into_gen_eval(self) -> ( SemiHonestTensorPreGen, SemiHonestTensorPreEval) {
        (
            SemiHonestTensorPreGen {
            n: self.n,
            m: self.m,
            chunking_factor: self.chunking_factor,
            delta: self.delta,
            x_labels: self.x_labels.iter().map(|share| share.gen_share).collect(),
            y_labels: self.y_labels.iter().map(|share| share.gen_share).collect(),
            alpha_labels: self.alpha_labels.iter().map(|share| share.gen_share).collect(),
            beta_labels: self.beta_labels.iter().map(|share| share.gen_share).collect(),
            },
        
            SemiHonestTensorPreEval {
                n: self.n,
                m: self.m,
                chunking_factor: self.chunking_factor,
                x_labels: self.x_labels.iter().map(|share| share.eval_share).collect(),
                y_labels: self.y_labels.iter().map(|share| share.eval_share).collect(),
            }
        )
    }

}

pub struct SemiHonestTensorPreGen {
    pub n: usize,
    pub m: usize,
    pub chunking_factor: usize,
    pub delta: Delta,
    pub x_labels: Vec<Block>,
    pub y_labels: Vec<Block>,
    pub alpha_labels: Vec<Block>,
    pub beta_labels: Vec<Block>,
}

pub struct SemiHonestTensorPreEval {
    pub n: usize,
    pub m: usize,
    pub chunking_factor: usize,
    pub x_labels: Vec<Block>,
    pub y_labels: Vec<Block>,
}