use crate::{
    aes::{FixedKeyAes, FIXED_KEY_AES},
    circuit::{Circuit, CircuitError, Gate},
    block::Block,
    macs::Mac,
    delta::Delta,
    fpre::{AuthBitShare, AuthTripleShare, FpreEval},
    circuit::{AuthHalfGate},
};

/// Errors that can occur during garbled circuit evaluation.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum AuthEvaluatorError {
    #[error(transparent)]
    CircuitError(#[from] CircuitError),
    #[error("evaluator not finished")]
    NotFinished,
    #[error("MAC verification failed at gate {0}")]
    MacCheckFailed(usize), 
    #[error("expected {expected} auth bits, got {actual}")]
    InvalidAuthBitCount { expected: usize, actual: usize },
    #[error("expected {expected} derandomization bits, got {actual}")]
    InvalidDerandCount { expected: usize, actual: usize },
    #[error("expected {expected} input MACs, got {actual}")]
    InvalidInputMacCount { expected: usize, actual: usize },
    #[error("expected {expected} masked inputs, got {actual}")]
    InvalidMaskedInputCount { expected: usize, actual: usize },
    #[error("expected {expected} output MACs, got {actual}")]
    InvalidOutputMacCount { expected: usize, actual: usize },
}

/// Evaluates an AND gate, returning the output label and masked value
#[inline]
pub(crate) fn and_gate(
    lx: Block,
    ly: Block,
    sx: AuthBitShare,
    sy: AuthBitShare,
    sz: AuthBitShare,
    ss: AuthBitShare,
    encrypted_gate: AuthHalfGate,
    za: bool,
    zb: bool,
    cipher: &FixedKeyAes,
    gid: usize,
) -> (Block, bool) {
    // add in Eval's share of the half gate
    let g_0 = encrypted_gate.gates[0] ^ sy.mac.as_block();
    let g_1 = encrypted_gate.gates[1] ^ sx.mac.as_block();

    let j = Block::new((gid as u128).to_be_bytes());
    let k = Block::new(((gid + 1) as u128).to_be_bytes());

    // hash the input labels
    let mut h = [lx, ly];
    cipher.tccr_many(&[j, k], &mut h);
    let [hx, hy] = h;

    // get eval's share of the output label
    let sz_mac = sz.mac.as_block();
    let ss_mac = ss.mac.as_block();
    
    // evaluate the gate and add in eval's share of the output label
    let lz = hx ^ hy ^ sz_mac ^ ss_mac ^ (g_0.mul_bool(za)) ^ ((g_1^lx).mul_bool(zb));
    let zc = lz.lsb() ^ encrypted_gate.mask;
    
    (lz, zc)
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
    // Start with combined share of sigma and z
    let mut share = (ss.mac.as_block() ^ ss.key.as_block() ^ delta.mul_bool(ss.bit())) ^
                   (sz.mac.as_block() ^ sz.key.as_block() ^ delta.mul_bool(sz.bit()));

    // Apply adjustments based on masked values
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

/// Output of the authenticated evaluator.
#[derive(Debug)]
pub struct AuthEvalOutput {
    /// Output labels of the circuit.
    pub output_labels: Vec<Mac>,
    /// Output auth bits of the circuit.
    pub output_auth_bits: Vec<AuthBitShare>,
    /// Authentication hash of the circuit.
    pub auth_hash: Block,
    /// Output values of the circuit.
    pub masked_output_values: Vec<bool>,
    /// All masked values of the circuit.
    pub masked_values: Vec<bool>,
}

/// Authenticated garbled circuit evaluator.
pub struct AuthEval {
    cipher: &'static FixedKeyAes,
    delta: Delta,
    labels: Vec<Block>,
    auth_bits: Vec<AuthBitShare>,
    triples: Vec<AuthTripleShare>,
    masked_values: Vec<bool>,
}

impl AuthEval {
    pub fn new() -> Self {
        Self {
            cipher: &(*FIXED_KEY_AES),
            delta: Delta::new(Block::ZERO),
            labels: Vec::new(),
            auth_bits: Vec::new(),
            triples: Vec::new(),
            masked_values: Vec::new(),
        }
    }

    pub fn new_with_pre(pre: FpreEval, circ: &Circuit) -> Self {
        Self {
            cipher: &(*FIXED_KEY_AES),
            delta: pre.delta_b,
            labels: vec![Block::ZERO; circ.inputs() + circ.outputs()],
            auth_bits: pre.wire_shares,
            triples: pre.triple_shares,
            masked_values: vec![false; circ.inputs() + circ.outputs()],
        }
    }

    /// Generates a consumer over the encrypted gates of a circuit, finish circuit dependent preprocessing
    pub fn evaluate<'a>(
        &'a mut self,
        circ: &'a Circuit,
        encrypted_gates: &[AuthHalfGate],
        input_labels: &[Block],
        masked_inputs: Vec<bool>,
    ) -> Result<(Vec<Block>, Vec<bool>), AuthEvaluatorError> {

        // copy the input labels into the labels vector
        for (i, label) in input_labels.iter().enumerate() {
            self.labels[i] = label.clone();
        }

        // copy the masked inputs into the masked values vector
        for (i, masked_input) in masked_inputs.iter().enumerate() {
            self.masked_values[i] = *masked_input;
        }

        let mut and_count = 0;
        let mut gid = 0;

        for gate in circ.gates() {
            match gate {
                Gate::Xor { x, y, z } => {
                    // compute the output wire auth bit from the input wire auth bits
                    self.auth_bits[z.id()] = self.auth_bits[x.id()] + self.auth_bits[y.id()];

                    // set the output wire label and masked value
                    self.labels[z.id()] = self.labels[x.id()] ^ self.labels[y.id()];
                    self.masked_values[z.id()] = self.masked_values[x.id()] ^ self.masked_values[y.id()];
                }
                Gate::And { x, y, z } => {
                    // Get labels for input wires
                    let lx = self.labels[x.id()];
                    let ly = self.labels[y.id()];
                    
                    // Get wire auth_bits
                    let sx = self.auth_bits[x.id()];
                    let sy = self.auth_bits[y.id()];
                    let sz = self.auth_bits[z.id()];

                    // Get AND of input auth_bits
                    let ss = self.triples[and_count].z;


                    // Get masked input bits
                    let za = self.masked_values[x.id()];
                    let zb = self.masked_values[y.id()];

                    // Evaluate the AND gate
                    let (lz, zc) = and_gate(
                        lx, 
                        ly, 
                        sx, 
                        sy, 
                        sz, 
                        ss, 
                        encrypted_gates[and_count], 
                        za, 
                        zb, 
                        self.cipher, 
                        gid
                    );

                    // Set output masked value and label
                    self.masked_values[z.id()] = zc;
                    self.labels[z.id()] = lz;

                    gid += 2;
                    and_count += 1;
                }
            }
        }
        Ok((self.labels.clone(), self.masked_values.clone()))
    }

    /// Compute the authentication hash for the evaluated circuit
    pub fn verify(
        &self,
        circ: &Circuit,
        masked_values: &[bool],
    ) -> Block {
        let mut auth_hash = Block::ZERO;
        let mut and_count = 0;
        
        for gate in circ.gates() {
            if let Gate::And { x, y, z } = gate {
                let ss = &self.triples[and_count].z;
                let sz = &self.auth_bits[z.id()];
                let sx = &self.auth_bits[x.id()];
                let sy = &self.auth_bits[y.id()];

                // Get masked values
                let za = masked_values[x.id()];
                let zb = masked_values[y.id()];
                let zc = masked_values[z.id()];

                let share = check_and(ss, sz, sx, sy, za, zb, zc, *self.delta.as_block());
                
                // Update hash                            
                auth_hash ^= self.cipher.tccr(Block::new((and_count as u128).to_be_bytes()), share);
                and_count += 1;
            }
        }
        
        auth_hash
    }
}