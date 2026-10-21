use smplx_sdk::transaction::FinalTransaction;

use crate::fuzz::mutations::Mutation;

/// Where each UTXO of the assembled transaction sits after the mutations.
#[derive(Clone, Debug)]
pub(crate) struct UtxoMap {
    inputs: Vec<usize>,
    outputs: Vec<usize>,
}

impl UtxoMap {
    pub(crate) fn new(transaction: &FinalTransaction, mutations: &[Mutation]) -> Self {
        let mut map = Self {
            inputs: (0..transaction.n_inputs()).collect(),
            outputs: (0..transaction.n_outputs()).collect(),
        };

        for mutation in mutations {
            map.follow(mutation);
        }

        map
    }

    /// Moves the UTXOs `mutation` reorders.
    pub(crate) fn follow(&mut self, mutation: &Mutation) {
        match *mutation {
            Mutation::SwapInputs(first, second) => Self::swap(&mut self.inputs, first, second),
            Mutation::SwapOutputs(first, second) => Self::swap(&mut self.outputs, first, second),
            _ => {}
        }
    }

    /// Where input `old_index` of the assembled transaction sits now.
    pub(crate) fn new_input_position(&self, old_index: usize) -> usize {
        self.inputs[old_index]
    }

    /// Where output `old_index` of the assembled transaction sits now.
    pub(crate) fn new_output_position(&self, old_index: usize) -> usize {
        self.outputs[old_index]
    }

    /// The index in the assembled transaction of the input now at `new_position`, `None` for an added one.
    pub(crate) fn old_input_index(&self, new_position: usize) -> Option<usize> {
        self.inputs.iter().position(|input| *input == new_position)
    }

    /// The index in the assembled transaction of the output now at `new_position`, `None` for an added one.
    pub(crate) fn old_output_index(&self, new_position: usize) -> Option<usize> {
        self.outputs.iter().position(|output| *output == new_position)
    }

    fn swap(positions: &mut [usize], first: usize, second: usize) {
        for position in positions {
            if *position == first {
                *position = second;
            } else if *position == second {
                *position = first;
            }
        }
    }
}
