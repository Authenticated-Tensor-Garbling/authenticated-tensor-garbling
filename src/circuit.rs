use std::ops::Index;

use crate::block::Block;
use serde::{Deserialize, Serialize};

use crate::DEFAULT_BATCH_SIZE;

#[derive(Debug, Clone)]
pub struct Wire {
    pub id: usize,
}

impl Wire {
    pub fn id(&self) -> usize { self.id }
}

#[derive(Debug, Clone)]
pub enum Gate {
    And { x: Wire, y: Wire, z: Wire },
    Xor { x: Wire, y: Wire, z: Wire },
}

#[derive(Debug, Clone)]
pub struct Circuit {
    pub gates: Vec<Gate>,
    pub and_count: usize,
    pub xor_count: usize,
}

impl Circuit {
    pub fn inputs(&self) -> usize { (self.and_count + self.xor_count) * 2}  // every gate has 2 inputs
    pub fn outputs(&self) -> usize { self.and_count + self.xor_count }      // every gate has 1 output
    pub fn gates(&self) -> &[Gate] { &self.gates }
    pub fn and_count(&self) -> usize { self.and_count }
    pub fn xor_count(&self) -> usize { self.xor_count }

    pub fn from_params(num_xor: usize, num_and: usize) -> Self {
        let mut gates = Vec::with_capacity(num_xor + num_and);
        for i in 0..num_xor {
            gates.push(Gate::Xor { x: Wire { id: i * 3 }, y: Wire { id: i * 3 + 1 }, z: Wire { id: i * 3 + 2 } });
        }
        for i in num_xor..(num_xor + num_and) {
            gates.push(Gate::And { x: Wire { id: i * 3 }, y: Wire { id: i * 3 + 1 }, z: Wire { id: i * 3 + 2 } });
        }
        Self {
            gates: gates,
            and_count: num_and,
            xor_count: num_xor,
        }
    }

    pub fn get_input_wires(&self) -> Vec<Wire> {
        let mut input_wires = Vec::new();
        for gate in self.gates.iter() {
            match gate {
                Gate::And { x, y, z: _ } => {input_wires.push(x.clone()); input_wires.push(y.clone());},
                Gate::Xor { x, y, z: _ } => {input_wires.push(x.clone()); input_wires.push(y.clone());},
            }
        }
        input_wires
    }

    pub fn get_xor_input_wires(&self) -> Vec<Wire> {
        let mut xor_wires = Vec::new();
        for gate in self.gates.iter() {
            match gate {
                Gate::Xor { x, y, z: _} => {xor_wires.push(x.clone()); xor_wires.push(y.clone());},
                _ => {},
            }
        }
        xor_wires
    }

    pub fn get_and_wires(&self) -> Vec<Wire> {
        let mut and_wires = Vec::new();
        for gate in self.gates.iter() {
            match gate {
                Gate::And { x, y, z} => {and_wires.push(x.clone()); and_wires.push(y.clone()); and_wires.push(z.clone());},
                _ => {},
            }
        }
        and_wires
    }

    pub fn get_output_wires(&self) -> Vec<Wire> {
        let mut output_wires = Vec::new();
        for gate in self.gates.iter() {
            match gate {
                Gate::And { x: _, y: _, z: z } => output_wires.push(z.clone()),
                Gate::Xor { x: _, y: _, z: z } => output_wires.push(z.clone()),
            }
        }
        output_wires
    }
    
    pub fn total_wires(&self) -> usize {
        self.gates.len() * 3
    }
}

/// An error that can occur when performing operations with a circuit.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum CircuitError {
    #[error("Invalid number of wires: expected {0}, got {1}")]
    InvalidWireCount(usize, usize),
    #[error("Invalid input length: expected {expected}, got {actual}")]
    InvalidInputLength { expected: usize, actual: usize },
    #[error("Invalid output length: expected {expected}, got {actual}")]
    InvalidOutputLength { expected: usize, actual: usize },
}

/// A garbled circuit.
#[derive(Debug, Clone)]
pub struct GarbledCircuit {
    /// Encrypted gates.
    pub gates: Vec<EncryptedGate>,
}

/// A garbled circuit.
#[derive(Debug, Clone)]
pub struct AuthGarbledCircuit {
    /// Encrypted gates.
    pub gates: Vec<AuthHalfGate>,
}

/// Encrypted gate truth table
///
/// For the half-gate garbling scheme a truth table will typically have 2 rows,
/// except for in privacy-free garbling mode where it will be reduced to 1.
///
/// We do not yet support privacy-free garbling.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EncryptedGate(#[serde(with = "serde_arrays")] pub(crate) [Block; 2]);

impl EncryptedGate {
    pub(crate) fn new(inner: [Block; 2]) -> Self {
        Self(inner)
    }
}

impl Index<usize> for EncryptedGate {
    type Output = Block;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

/// A batch of encrypted gates.
///
/// # Parameters
///
/// - `N`: The size of a batch.
#[derive(Debug, Serialize, Deserialize)]
pub struct EncryptedGateBatch<const N: usize = DEFAULT_BATCH_SIZE>(
    #[serde(with = "serde_arrays")] [EncryptedGate; N],
);

impl<const N: usize> EncryptedGateBatch<N> {
    /// Creates a new batch of encrypted gates.
    pub fn new(batch: [EncryptedGate; N]) -> Self {
        Self(batch)
    }

    /// Returns the inner array.
    pub fn into_array(self) -> [EncryptedGate; N] {
        self.0
    }
}

/// An authenticated half gate
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AuthHalfGate {
    #[serde(with = "serde_arrays")]
    pub(crate) gates: [Block; 2],
    pub(crate) mask: bool,
}

impl AuthHalfGate {
    /// Creates a new authenticated half gate
    pub fn new(gates: [Block; 2], mask: bool) -> Self {
        Self { gates, mask }
    }
}

/// A batch of authenticated half gates.
///
/// # Parameters
///
/// - `N`: The size of a batch.
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthHalfGateBatch<const N: usize = DEFAULT_BATCH_SIZE>(
    #[serde(with = "serde_arrays")] [AuthHalfGate; N],
);

impl<const N: usize> AuthHalfGateBatch<N> {
    /// Creates a new batch of authenticated half gates.
    pub fn new(batch: [AuthHalfGate; N]) -> Self {
        Self(batch)
    }

    /// Returns the inner array.
    pub fn into_array(self) -> [AuthHalfGate; N] {
        self.0
    }
}

mod tests {
    use super::*;

    /// Test that the every wire id is unique and that the circuit has the correct number of inputs, outputs, and gates
    #[test]
    fn test_from_params() {
        let circ = Circuit::from_params(2, 2);

        // Track which wire IDs we've seen
        let mut seen_ids = std::collections::HashSet::new();
        
        for gate in circ.gates.iter() {
            let (x, y, z) = match gate {
                Gate::And { x, y, z } => (x, y, z),
                Gate::Xor { x, y, z } => (x, y, z),
            };
            
            // Each wire ID should only appear once
            assert!(seen_ids.insert(x.id()), "Duplicate wire ID: {}", x.id());
            assert!(seen_ids.insert(y.id()), "Duplicate wire ID: {}", y.id());
            assert!(seen_ids.insert(z.id()), "Duplicate wire ID: {}", z.id());
        }
        assert_eq!(circ.inputs(), 8);
        assert_eq!(circ.outputs(), 4);
        assert_eq!(circ.gates.len(), 4);
        assert_eq!(circ.and_count(), 2);
        assert_eq!(circ.xor_count(), 2);
    }

    /// Test that all wire IDs are contiguous starting from 0
    #[test]
    fn test_wire_ids_contiguous() {
        let circ = Circuit::from_params(3, 4);
        
        // Collect all wire IDs
        let mut wire_ids = Vec::new();
        for gate in circ.gates.iter() {
            let (x, y, z) = match gate {
                Gate::And { x, y, z } => (x, y, z),
                Gate::Xor { x, y, z } => (x, y, z),
            };
            wire_ids.push(x.id());
            wire_ids.push(y.id());
            wire_ids.push(z.id());
        }
        
        // Check that they are contiguous starting from 0
        for (index, &wire_id) in wire_ids.iter().enumerate() {
            assert_eq!(wire_id, index, "Wire ID {} should be at index {}", wire_id, index);
        }
        
        // Verify the total count matches expected
        let total_gates = circ.and_count() + circ.xor_count();
        let expected_wire_count = total_gates * 3; // Each gate has 3 wires (x, y, z)
        assert_eq!(wire_ids.len(), expected_wire_count);
    }
}