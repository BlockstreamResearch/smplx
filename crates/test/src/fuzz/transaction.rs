use simplicityhl::num::U256;

use smplx_sdk::transaction::FinalTransaction;
use smplx_sdk::utils::hash_script;

/// What guided generation draws from the scenario's transaction.
#[derive(Debug)]
pub(crate) struct FuzzTransactionSchema {
    /// Amounts, assets and script hashes of the inputs and outputs.
    pub(crate) literals: Vec<U256>,
    /// The larger of the input and output counts.
    pub(crate) utxos_max: u32,
}

impl FuzzTransactionSchema {
    pub(crate) fn of(transaction: &FinalTransaction) -> Self {
        let inputs = transaction.inputs().iter().map(|input| {
            let input = &input.partial_input;

            (input.amount, input.asset, &input.witness_utxo.script_pubkey)
        });
        let outputs = transaction
            .outputs()
            .iter()
            .map(|output| (Some(output.amount), Some(output.asset), &output.script_pubkey));

        let mut literals = Vec::new();

        for (amount, asset, script) in inputs.chain(outputs) {
            literals.extend(amount.map(U256::from));
            literals.extend(asset.map(|asset| U256::from_byte_array(asset.to_byte_array())));

            if !script.is_empty() {
                literals.push(U256::from_byte_array(hash_script(script)));
            }
        }

        Self {
            literals,
            utxos_max: u32::try_from(transaction.n_inputs().max(transaction.n_outputs())).unwrap_or(u32::MAX),
        }
    }
}
