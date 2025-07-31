// TODO: Move all of this code elsewhere, maybe into correlated.rs. This code is not used in the auth garbling implementation right now.
// Mostly helpful as a reference for understanding the function independent preprocessing of auth garbling.

use std::ops::Add;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;

use crate::delta::Delta;
use crate::macs::Mac;
use crate::keys::Key;
use crate::block::Block;
use crate::circuit::Wire;

#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum FpreError {
    #[error("fpre not yet generated")]
    NotGenerated,
    #[error("invalid wire shares: have {0}, expected {1}")]
    InvalidWireCount(usize, usize),
    #[error("invalid triple shares: have {0}, expected {1}")]
    InvalidTripleCount(usize, usize),
}

/// AuthBitShare consisting of a bool and a (key, mac) pair
#[derive(Debug, Clone, Default, Copy)]
pub struct AuthBitShare {
    /// Key
    pub key: Key,
    /// MAC
    pub mac: Mac,
    /// Value
    pub value: bool,
}

impl AuthBitShare {
    /// Retrieves the embedded bit from the LSB of `mac`.
    #[inline]
    pub fn bit(&self) -> bool {
        self.value
    }

    /// Checks that `share.mac == share.key.auth(share.bit, delta)`.
    pub fn verify(&self, delta: &Delta) {
        let want = self.key.auth(self.bit(), delta);
        assert_eq!(self.mac, want, "MAC mismatch in share");
    }
}

impl Add<AuthBitShare> for AuthBitShare {
    type Output = Self;

    #[inline]
    fn add(self, rhs: AuthBitShare) -> Self {
        Self {
            key: self.key + rhs.key,
            mac: self.mac + rhs.mac,
            value: self.value ^ rhs.value,
        }
    }
}

impl Add<&AuthBitShare> for AuthBitShare {
    type Output = Self;

    #[inline]
    fn add(self, rhs: &AuthBitShare) -> Self {
        Self {
            key: self.key + rhs.key,
            mac: self.mac + rhs.mac,
            value: self.value ^ rhs.value,
        }
    }
}

impl Add<AuthBitShare> for &AuthBitShare {
    type Output = AuthBitShare;

    #[inline]
    fn add(self, rhs: AuthBitShare) -> AuthBitShare {
        AuthBitShare {
            key: self.key + rhs.key,
            mac: self.mac + rhs.mac,
            value: self.value ^ rhs.value,
        }
    }
}

impl Add<&AuthBitShare> for &AuthBitShare {
    type Output = AuthBitShare;

    #[inline]
    fn add(self, rhs: &AuthBitShare) -> AuthBitShare {
        AuthBitShare {
            key: self.key + rhs.key,
            mac: self.mac + rhs.mac,
            value: self.value ^ rhs.value,
        }
    }
}

/// Builds one `AuthBitShare` from a bit and delta, ensuring `key.lsb()==false`.
fn build_share(rng: &mut ChaCha12Rng, bit: bool, delta: &Delta) -> AuthBitShare {
    let key: Key = rng.random();
    let mac = key.auth(bit, delta);
    AuthBitShare { key, mac, value: bit }
}

/// Represents an auth bit [x] = [r]+[s] where [r] is known to gen, auth by eval and [s] is known to eval, auth by gen.
#[derive(Debug, Clone, Default)]
pub struct AuthBit {
    /// Generator's share of the auth bit
    pub gen_share: AuthBitShare,  
    /// Evaluator's share of the auth bit
    pub eval_share: AuthBitShare,
}

impl AuthBit {
    /// Recover the full bit x = r ^ s
    pub fn full_bit(&self) -> bool {
        self.gen_share.bit() ^ self.eval_share.bit()
    }

    /// verify auth bits
    pub fn verify(&self, delta_a: &Delta, delta_b: &Delta) {
        // Reconstruct shares for testing
        let r = AuthBitShare {
            key: self.eval_share.key,
            mac: self.gen_share.mac,
            value: self.gen_share.bit(),
        };
        let s = AuthBitShare {
            key: self.gen_share.key,
            mac: self.eval_share.mac,
            value: self.eval_share.bit(),
        };
        r.verify(delta_b);
        s.verify(delta_a);
    }
}

/// A triple ([x], [y], [z]) of auth bits such that z = x & y.
#[derive(Debug, Clone)]
pub struct AuthTriple {
    /// x component of the triple
    pub x: AuthBit,
    /// y component of the triple
    pub y: AuthBit,
    /// z component of the triple
    pub z: AuthBit,
}

impl AuthTriple {
    /// verify auth triples
    pub fn verify(&self, delta_a: &Delta, delta_b: &Delta) {
        let x = self.x.full_bit();
        let y = self.y.full_bit();
        let z = self.z.full_bit();
        assert_eq!(z, x && y, "z must equal x & y");
        self.x.verify(delta_a, delta_b);
        self.y.verify(delta_a, delta_b);
        self.z.verify(delta_a, delta_b);
    }
}

/// Per-party triple share: x,y,z each an `AuthBitShare`.
#[derive(Debug, Clone)]
pub struct AuthTripleShare {
    /// x component of the triple
    pub x: AuthBitShare,
    /// y component of the triple
    pub y: AuthBitShare,
    /// z component of the triple
    pub z: AuthBitShare,
}

/// Insecure ideal Fpre that pre-generates auth bits for wires and auth triples for AND gates.
#[derive(Debug)]
pub struct Fpre {
    rng: ChaCha12Rng,
    /// XOR input wires (will need an authbit each) 
    xor_input_wires: Vec<Wire>,
    /// AND wires (will need an authbit for output, triple for all)
    and_wires: Vec<Wire>,
    /// Evaluator's global correlation
    delta_a: Delta,
    /// Generator's global correlation
    delta_b: Delta,

    /// Bits for wires (input + AND-output)
    pub auth_bits: Vec<AuthBit>,
    /// Triples for AND gates
    pub auth_triples: Vec<AuthTriple>,
}

impl Fpre {
    /// Creates a new Fpre with random `delta_a`, `delta_b`.
    pub fn new(seed: u64, xor_input_wires: Vec<Wire>, and_wires: Vec<Wire>) -> Self {
        let mut rng = ChaCha12Rng::seed_from_u64(seed);

        let delta_a: Delta = Delta::random(&mut rng).set_lsb(true);
        let delta_b: Delta = Delta::random(&mut rng).set_lsb(false);

        Self {
            rng,
            xor_input_wires,
            and_wires,
            delta_a,
            delta_b,
            auth_bits: Vec::new(),
            auth_triples: Vec::new(),
        }
    }

    /// Builds an AuthBit [x] from a bit b such that x=b 
    pub fn gen_auth_bit(&mut self, x: bool) -> AuthBit {
        
        let r = self.rng.random_bool(0.5);
        let s = x ^ r;

        let r_share = build_share(&mut self.rng, r, &self.delta_b);
        let s_share = build_share(&mut self.rng, s, &self.delta_a);

        AuthBit {
            // Swapped key/mac for each share so that
            // gen knows mac from delta_b and key from delta_a, etc.
            gen_share: AuthBitShare{ mac: r_share.mac, key: s_share.key, value: r},
            eval_share: AuthBitShare{mac: s_share.mac, key: r_share.key, value: s},
        }
    }

    /// Builds a random triple
    pub fn gen_auth_triple(&mut self) -> AuthTriple {
        let x = self.rng.random_bool(0.5);
        let y = self.rng.random_bool(0.5);
        let z = x && y;

        AuthTriple {
            x: self.gen_auth_bit(x),
            y: self.gen_auth_bit(y),
            z: self.gen_auth_bit(z),
        }
    }

    /// Main Fpre generation: auth bits for wires (input + AND) and triples for AND gates
    pub fn generate(&mut self) {
        
        // TODO This is a hotfix to align the number of auth bits with the number of wires
        // the wires that shouldn't be initialized (for example XOR outputs) are overwritten
        // let total_wire_bits = self.num_input + self.num_output; //+ self.num_and;
        // TODO reserve the correct size from the circuit size self.auth_bits.reserve(total_wire_bits);

        let total_wire_bits = self.and_wires.len() + self.xor_input_wires.len() + self.xor_input_wires.len()/2;
        self.auth_bits.resize(total_wire_bits, AuthBit::default());

        for wire in self.xor_input_wires.clone().iter() {
            let wire_index = wire.id();
            let x = self.rng.random_bool(0.5);
            let auth_bit = self.gen_auth_bit(x);
            self.auth_bits[wire_index] = auth_bit;
        }
        println!("auth_bits after xor: {:?}", self.auth_bits.len());

        for i in 0..(self.and_wires.len()/3) {
            let i = i * 3;
            let x_index = self.and_wires[i].id();
            let y_index = self.and_wires[i+1].id();
            let z_index = self.and_wires[i+2].id();

            let triple = self.gen_auth_triple();
            let x = self.rng.random_bool(0.5);
            let auth_bit  = self.gen_auth_bit(x);

            self.auth_bits[x_index] = triple.x.clone();
            self.auth_bits[y_index] = triple.y.clone();
            self.auth_bits[z_index] = auth_bit;
            self.auth_triples.push(triple);
        }
        println!("auth_bits after and: {:?}", self.auth_bits.len());
    }
    
    /// Returns a reference to the generator's global correlation.
    pub fn delta_a(&self) -> &Delta {
        &self.delta_a
    }

    /// Returns a reference to the evaluator's global correlation.
    pub fn delta_b(&self) -> &Delta {
        &self.delta_b
    }

    /// Consumes `self` to produce `(FpreGen, FpreEval)` ownership in one go.
    pub fn into_gen_eval(mut self) -> (FpreGen, FpreEval) {

        // Generator wire shares
        let gen_wire_shares = self.auth_bits
            .iter()
            .map(|bit| bit.gen_share.clone())
            .collect();

        // Evaluator wire shares
        let eval_wire_shares = self.auth_bits
            .drain(..)
            .map(|bit| bit.eval_share)
            .collect::<Vec<_>>();

        // Generator triple shares
        let gen_triple_shares = self.auth_triples
            .iter()
            .map(|t| AuthTripleShare {
                x: t.x.gen_share.clone(),
                y: t.y.gen_share.clone(),
                z: t.z.gen_share.clone(),
            })
            .collect();

        // Evaluator triple shares
        let eval_triple_shares = self.auth_triples
            .drain(..)
            .map(|t| AuthTripleShare {
                x: t.x.eval_share,
                y: t.y.eval_share,
                z: t.z.eval_share,
            })
            .collect();

        let gb: FpreGen = FpreGen {
            num_input: self.xor_input_wires.len() + (self.and_wires.len() * 2 / 3),
            num_output: (self.xor_input_wires.len() / 2) + (self.and_wires.len() / 3),
            num_and: self.and_wires.len()/3,
            delta_a: self.delta_a,
            wire_shares: gen_wire_shares,
            triple_shares: gen_triple_shares,
        };

        let eval: FpreEval = FpreEval {
            num_input: self.xor_input_wires.len() + (self.and_wires.len() * 2 / 3),
            num_output: (self.xor_input_wires.len() / 2) + (self.and_wires.len() / 3),
            num_and: self.and_wires.len()/3,
            delta_b: self.delta_b,
            wire_shares: eval_wire_shares,
            triple_shares: eval_triple_shares,
        };

        (gb, eval)
    }
}

/// Fpre data from the generator's perspective.
#[derive(Debug)]
pub struct FpreGen {
    /// number of input wires
    pub num_input: usize,
    /// number of output wires
    pub num_output: usize,
    /// number of AND gates
    pub num_and: usize,
    /// generator's global correlation
    pub delta_a: Delta,
    /// wire shares
    pub wire_shares: Vec<AuthBitShare>,
    /// triple shares
    pub triple_shares: Vec<AuthTripleShare>,
}

/// Fpre data from the evaluator's perspective.
#[derive(Debug)]
pub struct FpreEval {
    /// number of input wires
    pub num_input: usize,
    /// number of output wires
    pub num_output: usize,
    /// number of AND gates
    pub num_and: usize,
    /// evaluator's global correlation
    pub delta_b: Delta,
    /// wire shares
    pub wire_shares: Vec<AuthBitShare>,
    /// triple shares
    pub triple_shares: Vec<AuthTripleShare>,
}

#[cfg(test)]
mod tests {

    use super::*;

    use std::iter::zip;
    use crate::circuit::Circuit;

    #[test]
    fn test_fpre_insecure() {
        let num_xor = 25;
        let num_and = 50;

        let circ = Circuit::from_params(num_xor, num_and);

        println!("xor_input_wires: {:?}", circ.get_xor_input_wires().len());
        println!("and_wires: {:?}", circ.get_and_wires().len());

        let mut fpre = Fpre::new(5, circ.get_xor_input_wires(), circ.get_and_wires());
        fpre.generate();

        // should have auth bits for all inputs and AND outputs
        // BAD CODE -- should be using Options and seeing what is genuinely populated. Or what isn't ::Default()
        let anticipated_auth_bits = num_xor * 3 + num_and * 3;
        // should have one triple per gate
        let anticipated_auth_triples = num_and;

        assert_eq!(fpre.auth_bits.len(), anticipated_auth_bits);
        assert_eq!(fpre.auth_triples.len(), anticipated_auth_triples);

        for bit in &fpre.auth_bits {
            bit.verify(fpre.delta_a(), fpre.delta_b());
        }
        for triple in &fpre.auth_triples {
            triple.verify(fpre.delta_a(), fpre.delta_b());
        }

        let (fpre_gen, fpre_eval) = fpre.into_gen_eval();

        // wire shares length
        assert_eq!(
            fpre_gen.wire_shares.len(),
            anticipated_auth_bits
        );
        assert_eq!(
            fpre_eval.wire_shares.len(),
            anticipated_auth_bits
        );

        // triple shares length
        assert_eq!(fpre_gen.triple_shares.len(), anticipated_auth_triples);
        assert_eq!(fpre_eval.triple_shares.len(), anticipated_auth_triples);

        // Check generator/evaluator shares align
        for (gen_share, eval_share) in zip(fpre_gen.wire_shares, fpre_eval.wire_shares) {
            let bit = AuthBit {
                gen_share,
                eval_share,
            };
            bit.verify(&fpre_gen.delta_a, &fpre_eval.delta_b);
        }

        for (gen_triple, eval_triple) in zip(fpre_gen.triple_shares, fpre_eval.triple_shares) {
            let triple = AuthTriple {
                x: AuthBit {
                    gen_share: gen_triple.x,
                    eval_share: eval_triple.x,
                },
                y: AuthBit {
                    gen_share: gen_triple.y,
                    eval_share: eval_triple.y,
                },
                z: AuthBit {
                    gen_share: gen_triple.z,
                    eval_share: eval_triple.z,
                },
            };
            triple.verify(&fpre_gen.delta_a, &fpre_eval.delta_b);
        }
    }
}