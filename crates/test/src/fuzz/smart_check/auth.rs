use std::collections::HashMap;
use std::sync::Arc;

use simplicityhl::num::U256;
use simplicityhl::value::ValueConstructible;
use simplicityhl::{Value, WitnessNameToValueMap, WitnessValues};

use smplx_sdk::signer::Signer;
use smplx_sdk::transaction::{FinalTransaction, RequiredSignature, SigMessage};
use smplx_sdk::utils::random_mnemonic;

use crate::fuzz::execution::SignedTransaction;
use crate::fuzz::outcome::SetupError;

/// Checks that an accepted spend depends on its signatures.
pub(crate) struct AuthCheck<'a> {
    signer: &'a Signer,
    transaction: &'a FinalTransaction,
    signed_inputs: Vec<(usize, &'a str)>,
}

impl<'a> AuthCheck<'a> {
    /// `None` when no program input requests a signature. Each signed input comes with its signature's witness name.
    pub(crate) fn new(signer: &'a Signer, transaction: &'a FinalTransaction) -> Option<Self> {
        let signed_inputs: Vec<(usize, &str)> = transaction
            .inputs()
            .iter()
            .enumerate()
            .filter(|(_, input)| input.program_input.is_some())
            .filter_map(|(index, input)| match &input.required_sig {
                RequiredSignature::Witness(name)
                | RequiredSignature::WitnessWithPath(name, _)
                | RequiredSignature::WitnessWithMessage(name, _, _) => Some((index, name.as_str())),
                RequiredSignature::None | RequiredSignature::NativeEcdsa => None,
            })
            .collect();

        (!signed_inputs.is_empty()).then_some(Self {
            signer,
            transaction,
            signed_inputs,
        })
    }

    /// The bypasses found, empty when every variant is rejected.
    ///
    /// # Errors
    /// Returns a [`SetupError`] when a variant cannot be signed or executed.
    pub(crate) fn run(&self) -> Result<Vec<String>, SetupError> {
        let attacker_signer = self.attacker_signer();

        let wrong_key = self.accepted_inputs(&attacker_signer, &self.with_attacker_key(&attacker_signer))?;
        let replayed = self.accepted_inputs(self.signer, &self.replayed())?;

        let mut bypasses = Vec::new();

        for &(index, name) in &self.signed_inputs {
            let bypass = match (wrong_key.contains(&index), replayed.contains(&index)) {
                (true, true) => format!("input {index} accepts any `{name}`, the signature is not checked"),
                (true, false) => format!("input {index} accepts a `{name}` signature from any key"),
                (false, true) => format!(
                    "input {index} accepts a `{name}` signature made for another transaction, so it can be replayed"
                ),
                (false, false) => continue,
            };

            bypasses.push(bypass);
        }

        Ok(bypasses)
    }

    fn attacker_signer(&self) -> Signer {
        Signer::from_mnemonic(&random_mnemonic(), *self.signer.get_network())
    }

    /// The attacker also swaps any key the contract reads from the witness.
    fn with_attacker_key(&self, attacker_signer: &Signer) -> FinalTransaction {
        let own_key = Self::key_value(self.signer);
        let attacker_key = Self::key_value(attacker_signer);

        let mut transaction = self.transaction.clone();

        for program_input in transaction
            .inputs_mut()
            .iter_mut()
            .filter_map(|input| input.program_input.as_mut())
        {
            let witness: HashMap<_, _> = program_input
                .witness
                .iter()
                .map(|(name, value)| {
                    let value = if *value == own_key {
                        attacker_key.clone()
                    } else {
                        value.clone()
                    };

                    (name.clone(), value)
                })
                .collect();

            program_input.witness = WitnessValues::from_map(witness);
        }

        transaction
    }

    /// Each signed input signs what it would sign for a transaction with a different sighash.
    fn replayed(&self) -> FinalTransaction {
        let mut transaction = self.transaction.clone();

        for &(index, _) in &self.signed_inputs {
            let input = &mut transaction.inputs_mut()[index];

            let (name, path, message) = match &input.required_sig {
                RequiredSignature::Witness(name) => (name.clone(), Vec::new(), SigMessage::Sighash),
                RequiredSignature::WitnessWithPath(name, path) => (name.clone(), path.clone(), SigMessage::Sighash),
                RequiredSignature::WitnessWithMessage(name, path, message) => {
                    (name.clone(), path.clone(), message.clone())
                }
                RequiredSignature::None | RequiredSignature::NativeEcdsa => continue,
            };

            let other_message = SigMessage::Custom(Arc::new(move |sighash: [u8; 32]| {
                message.digest(sighash.map(|byte| !byte))
            }));

            input.required_sig = RequiredSignature::WitnessWithMessage(name, path, other_message);
        }

        transaction
    }

    fn accepted_inputs(&self, signer: &Signer, transaction: &FinalTransaction) -> Result<Vec<usize>, SetupError> {
        let signed = SignedTransaction::new(signer, transaction)?;
        let mut accepted = Vec::new();

        for &(index, _) in &self.signed_inputs {
            if signed.run(index)?.is_accepted() {
                accepted.push(index);
            }
        }

        Ok(accepted)
    }

    fn key_value(signer: &Signer) -> Value {
        Value::u256(U256::from_byte_array(signer.get_schnorr_public_key().serialize()))
    }
}

#[cfg(test)]
mod tests {
    use simplicityhl::{Arguments, TemplateProgramWitness};

    use smplx_sdk::program::Program;
    use smplx_sdk::provider::SimplicityNetwork;
    use smplx_sdk::transaction::{PartialInput, ProgramInput, UTXO};

    use crate::config::DEFAULT_TEST_MNEMONIC;

    use super::*;

    struct Contract {
        source: &'static str,
        key_argument: bool,
        key_witness: bool,
        required_sig: RequiredSignature,
    }

    impl Contract {
        fn bypasses(self) -> Vec<String> {
            let signer = Signer::from_mnemonic(DEFAULT_TEST_MNEMONIC, SimplicityNetwork::default_regtest());
            let key = AuthCheck::key_value(&signer);

            let arguments = if self.key_argument {
                Arguments::from_map(HashMap::from([(
                    TemplateProgramWitness::parameter_from_str("KEY"),
                    key.clone(),
                )]))
            } else {
                Arguments::default()
            };
            let witness = if self.key_witness {
                WitnessValues::from_map(HashMap::from([(TemplateProgramWitness::witness_from_str("KEY"), key)]))
            } else {
                WitnessValues::default()
            };

            let program = Program::new(self.source, arguments);

            let mut utxo = UTXO::default();
            utxo.txout.script_pubkey = program.get_script_pubkey(signer.get_network());

            let mut transaction = FinalTransaction::new();
            transaction.add_program_input(
                PartialInput::new(utxo),
                ProgramInput::new(Box::new(program), witness),
                self.required_sig,
            );

            let check = AuthCheck::new(&signer, &transaction).unwrap();

            assert_eq!(
                check.accepted_inputs(&signer, &transaction).unwrap(),
                vec![0],
                "the honest spend must be accepted"
            );

            check.run().unwrap()
        }
    }

    #[test]
    fn finds_each_kind_of_bypass() {
        let signature = || RequiredSignature::Witness("SIGNATURE".to_string());

        let p2pk = Contract {
            source: "fn main() { jet::bip_0340_verify((param::KEY, jet::sig_all_hash()), witness::SIGNATURE) }",
            key_argument: true,
            key_witness: false,
            required_sig: signature(),
        };
        assert!(p2pk.bypasses().is_empty());

        let unchecked = Contract {
            source: "fn main() { let sig: Signature = witness::SIGNATURE; }",
            key_argument: false,
            key_witness: false,
            required_sig: signature(),
        };
        assert!(unchecked.bypasses()[0].contains("the signature is not checked"));

        let key_from_witness = Contract {
            source: "fn main() { jet::bip_0340_verify((witness::KEY, jet::sig_all_hash()), witness::SIGNATURE) }",
            key_argument: false,
            key_witness: true,
            required_sig: signature(),
        };
        assert!(key_from_witness.bypasses()[0].contains("from any key"));

        let constant_message = Contract {
            source: "fn main() { jet::bip_0340_verify((param::KEY, 0x0000000000000000000000000000000000000000000000000000000000000000), witness::SIGNATURE) }",
            key_argument: true,
            key_witness: false,
            required_sig: RequiredSignature::WitnessWithMessage(
                "SIGNATURE".to_string(),
                Vec::new(),
                SigMessage::Custom(Arc::new(|_| [0; 32])),
            ),
        };
        assert!(constant_message.bypasses()[0].contains("can be replayed"));
    }
}
