use crate::block::Block;

#[derive(Debug, Clone, Copy)]

pub struct InputSharing {
    pub gen_share: Block,
    pub eval_share: Block,
}

impl InputSharing {
    pub fn bit(&self) -> bool {
        if self.gen_share == self.eval_share {
            false
        } else {
            true
        }
    }
}