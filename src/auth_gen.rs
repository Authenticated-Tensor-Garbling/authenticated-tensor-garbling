use crate::aes::{FixedKeyAes, FIXED_KEY_AES};

use crate::fpre::{AuthBitShare, AuthTripleShare, FpreGen};

use crate::block::Block;
use crate::delta::Delta;
use crate::keys::Key;
use crate::circuit::AuthHalfGate;

use mpz_circuits::{Circuit, Gate};



#[inline]
pub(crate) fn and_gate(
    lx: &Block,
    ly: &Block,
    sx: &AuthBitShare,
    sy: &AuthBitShare,
    sz: &AuthBitShare,
    ss: &AuthBitShare,
    delta: &Block,
    cipher: &FixedKeyAes,  
    gid: usize,
) -> (AuthHalfGate, Block) {

    let lx1 = lx ^ delta;
    let ly1 = ly ^ delta;
    
    let j = Block::new((gid as u128).to_be_bytes());
    let k = Block::new(((gid + 1) as u128).to_be_bytes());
    
    let mut h = [*lx, *ly, lx1, ly1];
    cipher.tccr_many(&[j, k, j, k], &mut h);
    let [hx, hy, hx1, hy1] = h;
    
    let g_0 = hx ^ hx1 ^ sy.key.as_block() ^ delta.mul_bool(sy.bit());
              
    let g_1 = hy ^ hy1 ^ sx.key.as_block() ^ delta.mul_bool(sx.bit()) ^ lx;
    
    let lz = hx ^ hy ^ sz.key.as_block() ^ delta.mul_bool(sz.bit()) ^ 
            ss.key.as_block() ^ delta.mul_bool(ss.bit());
    
    let gates = [g_0, g_1];
    let mask = lz.lsb();
    
    (AuthHalfGate::new(gates, mask), lz)
}

#[inline]
pub(crate) fn check_and(
    ss: &AuthBitShare,
    sz: &AuthBitShare,
    sx: &AuthBitShare,
    sy: &AuthBitShare,
    za: bool,
    zb: bool,
    zc: bool,
    delta: Block,
) -> Block {

    let mut share = (ss.mac.as_block() ^ ss.key.as_block() ^ delta.mul_bool(ss.bit())) ^
                   (sz.mac.as_block() ^ sz.key.as_block() ^ delta.mul_bool(sz.bit()));

    if za {
        share = share ^ sy.mac.as_block() ^ sy.key.as_block() ^ 
        delta.mul_bool(sy.bit());
    }
    if zb {
        share = share ^ sx.mac.as_block() ^ sx.key.as_block() ^ 
        delta.mul_bool(sx.bit());
    }
    if (za && zb) != zc {
        share = share ^ delta;
    }
    share
}

/// Output of the authenticated generator.
#[derive(Debug)]
pub struct AuthGenOutput {
    /// Output labels of the circuit.
    pub output_labels: Vec<Key>,
    /// Output auth bits of the circuit.
    pub output_auth_bits: Vec<AuthBitShare>,
    /// Authentication hash of the circuit.
    pub auth_hash: Block,
}

/// Authenticated garbled circuit generator.
pub struct AuthGen {
    cipher: &'static FixedKeyAes,
    delta: Delta,
    labels: Vec<Block>, 
    auth_bits: Vec<AuthBitShare>,
    masked_values: Vec<bool>,
    triples: Vec<AuthTripleShare>,
}

impl AuthGen {
    /// Create a new AuthGen with seed from coin-tossing
    pub fn new() -> Self {
        Self {
            cipher: &(*FIXED_KEY_AES),
            delta: Delta::new(Block::ZERO),
            labels: Vec::new(),
            auth_bits: Vec::new(),
            masked_values: Vec::new(),
            triples: Vec::new(),
        }
    }

    /// Instantiate an AuthGen with pre-generated wire and triple shares
    /// Consumes the pre-generated wire and triple shares
    pub fn new_with_pre(pre: FpreGen) -> Self {

        // TODO this RNG should either be seeded or passed in
        let mut rng = rand::rng();

        // TODO this finds a random label for each wire, but we should keep only
        // the labels for the input wires initialized
        let mut labels = vec![Block::ZERO; pre.num_input + pre.num_and];
        for i in 0..(pre.num_input + pre.num_and) {
            labels[i] = Block::random(&mut rng);
        }
        Self {
            cipher: &(*FIXED_KEY_AES),
            delta: pre.delta_a,
            labels,
            auth_bits: pre.wire_shares,
            masked_values: Vec::new(),
            triples: pre.triple_shares,
        }
    }

    pub fn get_masked_inputs(&self) -> Vec<bool> {
        self.masked_values.clone()
    }

    /// Generates an iterator over the encrypted gates of a circuit, finish circuit dependent preprocessing
    pub fn generate<'a>(
        &'a mut self,
        circ: &'a Circuit,
    ) -> (Vec<Block>, Vec<AuthHalfGate>) {
        
        let mut garbled_gates = Vec::with_capacity(circ.and_count());
        let mut and_count: usize = 0;
        let mut gid: usize = 0;

        for gate in circ.gates() {
            match gate {
                Gate::Xor { x, y, z } => {
                    self.auth_bits[z.id()] = self.auth_bits[x.id()] + self.auth_bits[y.id()]; // TODO do this in preprocessing; have circuit dependent preprocessing
                    self.labels[z.id()] = self.labels[x.id()] ^ self.labels[y.id()];
                },
                Gate::And { x, y, z } => {
                    // Get zero labels for input wires
                    let lx = self.labels[x.id()];
                    let ly = self.labels[y.id()];

                    // Get input auth_bits
                    let sx = self.auth_bits[x.id()];
                    let sy = self.auth_bits[y.id()];
                    let sz = self.auth_bits[z.id()];

                    // Get AND of input auth_bits
                    let ss = self.triples[and_count].z;

                    // Garble the gate and compute output label
                    let (half_gate, lz) = and_gate(&lx, &ly, &sx, &sy, &sz, &ss, self.delta.as_block(), self.cipher, gid);
                    self.labels[z.id()] = lz;
                    
                    gid += 2;
                    and_count += 1;

                    garbled_gates.push(half_gate);
                },
                _ => {
                    panic!("Unsupported gate: {:?}", gate);
                }
            }
        }
        (self.labels.clone(), garbled_gates)
    }

    /// Verify the circuit execution using masked values from evaluator
    pub fn verify<'a>(
        &'a self,
        circ: &'a Circuit,
        masked_values: &[bool],
    ) -> Block {
        
        let mut auth_hash = Block::ZERO;
        let mut and_count = 0;
        let delta = self.delta.as_block();
        
        for gate in circ.gates() {
            if let Gate::And { x, y, z } = gate {
                let ss = &self.triples[and_count].z;  // sigma share
                let sz = &self.auth_bits[z.id()];     // output auth bit
                let sx = &self.auth_bits[x.id()];     // input x auth bit
                let sy = &self.auth_bits[y.id()];     // input y auth bit

                // Get masked values from evaluator
                let za = masked_values[x.id()];
                let zb = masked_values[y.id()];
                let zc = masked_values[z.id()];

                let share = check_and(ss, sz, sx, sy, za, zb, zc, *delta);
                
                // Update hash                            
                auth_hash ^= self.cipher.tccr(Block::new((and_count as u128).to_be_bytes()), share);
                and_count += 1;
            }
        }

        auth_hash
    }
}

// /// Iterator over the encrypted gates of a circuit.
// pub struct AuthEncryptedGateIter<'a, I> {
//     /// Cipher to use to encrypt the gates.
//     cipher: &'static FixedKeyAes,
//     /// Global offset.
//     delta: Delta,
//     /// Buffer for the 0-bit labels.
//     labels: &'a mut [Block],
//     /// Buffer for the auth bits.
//     auth_bits: &'a mut [AuthBitShare],
//     /// Buffer for the sigma bits.
//     sigma_bits: &'a mut [AuthBitShare],
//     /// Buffer for the masked values.
//     masked_values: &'a mut [bool],
//     /// Iterator over the gates.
//     gates: I,
//     /// Iterator over the gates to check authenticity.
//     gates_check: I,
//     /// Circuit outputs.
//     outputs: Range<usize>,
//     /// Current gate id.
//     gid: usize,
//     /// Number of AND gates generated.
//     counter: usize,
//     /// Number of AND gates in the circuit.
//     and_count: usize,
//     /// Whether the entire circuit has been garbled.
//     complete: bool,
// }

// impl<'a, I> fmt::Debug for AuthEncryptedGateIter<'a, I> {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         write!(f, "AuthEncryptedGateIter {{ .. }}")
//     }
// }

// impl<'a, I> AuthEncryptedGateIter<'a, I>
// where
//     I: Iterator<Item = &'a Gate>,
// {
//     fn new(
//         delta: Delta,
//         gates: I,
//         gates_check: I,
//         outputs: Range<usize>,
//         labels: &'a mut [Block],
//         auth_bits: &'a mut [AuthBitShare],
//         sigma_bits: &'a mut [AuthBitShare],
//         masked_values: &'a mut [bool],
//         and_count: usize,
//     ) -> Self {
//         Self {
//             cipher: &(*FIXED_KEY_AES),
//             delta,
//             gates,
//             gates_check,
//             outputs,
//             labels,
//             auth_bits,
//             sigma_bits,
//             masked_values,
//             gid: 1,
//             counter: 0,
//             and_count,
//             complete: false,
//         }
//     }

//     /// Returns `true` if the generator has more encrypted gates to generate.
//     #[inline]
//     pub fn has_gates(&self) -> bool {
//         self.counter != self.and_count
//     }

//     /// Returns the encoded outputs of the circuit
//     pub fn finish(
//         mut self,
//         masked_values: Vec<bool>
//     ) -> Result<AuthGenOutput, AuthGeneratorError> {
//         if self.has_gates() {
//             return Err(AuthGeneratorError::NotFinished);
//         }

//         // Finish computing any "free" gates.
//         if !self.complete {
//             assert_eq!(self.next(), None);
//         }

//         let output_labels = Key::from_blocks(self.labels[self.outputs.clone()].to_vec());
//         let output_auth_bits = self.auth_bits[self.outputs.clone()].to_vec();

//         self.masked_values.copy_from_slice(&masked_values);

//         let mut auth_hash = Block::ZERO;
//         let mut and_count = 0;
//         let delta = self.delta.as_block();
        
//         for gate in self.gates_check {
//             if let Gate::And { x, y, z } = gate {
//                 let ss = &self.sigma_bits[and_count];
//                 let sz = &self.auth_bits[z.id()];
//                 let sx = &self.auth_bits[x.id()];
//                 let sy = &self.auth_bits[y.id()];

//                 // Get masked values
//                 let za = self.masked_values[x.id()];
//                 let zb = self.masked_values[y.id()];
//                 let zc = self.masked_values[z.id()];

//                 let share = check_and(ss, sz, sx, sy, za, zb, zc, *delta);
                
//                 // Update hash                            
//                 auth_hash ^= self.cipher.tccr(Block::new((and_count as u128).to_be_bytes()), share);
//                 and_count += 1;
//             }
//         }
        
//         Ok(AuthGenOutput { output_labels, output_auth_bits, auth_hash })
//     }
// }