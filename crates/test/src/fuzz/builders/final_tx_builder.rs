use std::collections::HashSet;

use simplicityhl::elements::Script;
use simplicityhl::{Arguments, WitnessValues};

use smplx_sdk::program::Program;
use smplx_sdk::transaction::{FinalTransaction, ProgramInput};

use crate::error::FuzzError;

type PostHook = dyn Fn(&mut FinalTransaction, &Script, &Arguments, &WitnessValues) -> Result<(), FuzzError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProgramTarget {
    Input(usize),
    Output(usize),
}

/// Object, which builds an initial transaction in a fuzzing step to make it valid after
/// generating of script hashes after program compiling.
pub struct FinalTransactionBuilder {
    initial_tx: FinalTransaction,
    targets: Vec<ProgramTarget>,
    post_hook: Option<Box<PostHook>>,
}

impl FinalTransactionBuilder {
    /// Creates a reusable fuzz transaction blueprint.
    ///
    /// # Errors
    /// Returns an error when there are no targets, a target is duplicated, or a target index is out of bounds.
    pub fn new(
        initial_tx: FinalTransaction,
        targets: impl IntoIterator<Item = ProgramTarget>,
    ) -> Result<Self, FuzzError> {
        let targets = targets.into_iter().collect::<Vec<_>>();

        Self::validate_targets(&initial_tx, &targets)?;

        Ok(Self {
            initial_tx,
            targets,
            post_hook: None,
        })
    }

    #[must_use]
    pub fn with_post_hook(
        mut self,
        hook: impl Fn(&mut FinalTransaction, &Script, &Arguments, &WitnessValues) -> Result<(), FuzzError> + 'static,
    ) -> Self {
        self.post_hook = Some(Box::new(hook));
        self
    }

    #[must_use]
    pub fn targets(&self) -> &[ProgramTarget] {
        &self.targets
    }

    /// Replaces the scriptPubKey at a selected input or output.
    ///
    /// # Errors
    /// Returns an error when the selected target index is out of bounds.
    pub fn set_program_script(
        tx: &mut FinalTransaction,
        target: ProgramTarget,
        script: &Script,
    ) -> Result<(), FuzzError> {
        match target {
            ProgramTarget::Input(index) => {
                let input_count = tx.n_inputs();
                let input = tx
                    .inputs_mut()
                    .get_mut(index)
                    .ok_or(FuzzError::InputTargetOutOfBounds { index, input_count })?;
                input.partial_input.witness_utxo.script_pubkey = script.clone();
            }
            ProgramTarget::Output(index) => {
                let output_count = tx.n_outputs();
                let output = tx
                    .outputs_mut()
                    .get_mut(index)
                    .ok_or(FuzzError::OutputTargetOutOfBounds { index, output_count })?;
                output.script_pubkey = script.clone();
            }
        }

        Ok(())
    }

    pub(crate) fn prepare_transaction(
        &self,
        program: &Program,
        script: &Script,
        arguments: &Arguments,
        witness: &WitnessValues,
    ) -> Result<FinalTransaction, FuzzError> {
        let mut tx = self.initial_tx.clone();

        for target in self.targets.iter().copied() {
            Self::set_program_script(&mut tx, target, script)?;

            if let ProgramTarget::Input(index) = target {
                let input_count = tx.n_inputs();
                let input = tx
                    .inputs_mut()
                    .get_mut(index)
                    .ok_or(FuzzError::InputTargetOutOfBounds { index, input_count })?;
                input.program_input = Some(ProgramInput::new(Box::new(program.clone()), witness.clone()));
            }
        }

        if let Some(post_hook) = &self.post_hook {
            post_hook(&mut tx, script, arguments, witness)?;
        }

        Ok(tx)
    }

    fn validate_targets(initial_tx: &FinalTransaction, targets: &[ProgramTarget]) -> Result<(), FuzzError> {
        if targets.is_empty() {
            return Err(FuzzError::NoProgramTargets);
        }

        let mut unique_targets = HashSet::with_capacity(targets.len());

        for target in targets.iter().copied() {
            if !unique_targets.insert(target) {
                return Err(FuzzError::DuplicateProgramTarget(target));
            }

            match target {
                ProgramTarget::Input(index) if index >= initial_tx.n_inputs() => {
                    return Err(FuzzError::InputTargetOutOfBounds {
                        index,
                        input_count: initial_tx.n_inputs(),
                    });
                }
                ProgramTarget::Output(index) if index >= initial_tx.n_outputs() => {
                    return Err(FuzzError::OutputTargetOutOfBounds {
                        index,
                        output_count: initial_tx.n_outputs(),
                    });
                }
                ProgramTarget::Input(_) | ProgramTarget::Output(_) => {}
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use simplicityhl::elements::hashes::Hash;
    use simplicityhl::elements::{OutPoint, Script, TxOut, Txid};

    use smplx_sdk::provider::SimplicityNetwork;
    use smplx_sdk::transaction::{PartialInput, PartialOutput, RequiredSignature, UTXO};

    use super::*;

    const DUMMY_PROGRAM: &str = r"
        fn main() {
            assert!(true);
        }
    ";

    fn initial_transaction() -> FinalTransaction {
        let network = SimplicityNetwork::default_regtest();
        let mut tx = FinalTransaction::new();

        for vout in 0..2 {
            let utxo = UTXO {
                outpoint: OutPoint::new(Txid::all_zeros(), vout),
                txout: TxOut::new_fee(1_000, network.policy_asset()),
                secrets: None,
            };
            tx.add_input(PartialInput::new(utxo), RequiredSignature::None);
            tx.add_output(PartialOutput::new(Script::new(), 500, network.policy_asset()));
        }

        tx
    }

    fn prepared_program() -> (Program, Script) {
        let program = Program::new(DUMMY_PROGRAM, Arguments::default());
        let script = program.get_script_pubkey(&SimplicityNetwork::default_regtest());
        (program, script)
    }

    #[test]
    fn rejects_empty_duplicate_and_out_of_bounds_targets() {
        assert_eq!(
            FinalTransactionBuilder::new(initial_transaction(), []).err(),
            Some(FuzzError::NoProgramTargets)
        );
        assert_eq!(
            FinalTransactionBuilder::new(
                initial_transaction(),
                [ProgramTarget::Input(0), ProgramTarget::Input(0)],
            )
            .err(),
            Some(FuzzError::DuplicateProgramTarget(ProgramTarget::Input(0)))
        );
        assert_eq!(
            FinalTransactionBuilder::new(initial_transaction(), [ProgramTarget::Output(2)]).err(),
            Some(FuzzError::OutputTargetOutOfBounds {
                index: 2,
                output_count: 2,
            })
        );
    }

    #[test]
    fn injects_program_into_all_targets_without_consuming_them() {
        let builder = FinalTransactionBuilder::new(
            initial_transaction(),
            [ProgramTarget::Input(0), ProgramTarget::Output(1)],
        )
        .unwrap();
        let (program, script) = prepared_program();
        let arguments = Arguments::default();
        let witness = WitnessValues::default();

        let first = builder
            .prepare_transaction(&program, &script, &arguments, &witness)
            .unwrap();
        let second = builder
            .prepare_transaction(&program, &script, &arguments, &witness)
            .unwrap();

        assert_eq!(builder.targets().len(), 2);
        assert!(first.inputs()[0].program_input.is_some());
        assert_eq!(first.inputs()[0].partial_input.witness_utxo.script_pubkey, script);
        assert_eq!(first.outputs()[1].script_pubkey, script);
        assert!(second.inputs()[0].program_input.is_some());
        assert_eq!(second.outputs()[1].script_pubkey, script);
    }

    #[test]
    fn runs_post_hook_after_program_injection_and_propagates_errors() {
        let hook_ran = Rc::new(Cell::new(false));
        let hook_ran_in_callback = Rc::clone(&hook_ran);
        let builder = FinalTransactionBuilder::new(initial_transaction(), [ProgramTarget::Input(0)])
            .unwrap()
            .with_post_hook(move |tx, script, _, _| {
                assert_eq!(tx.inputs()[0].partial_input.witness_utxo.script_pubkey, *script);
                hook_ran_in_callback.set(true);
                FinalTransactionBuilder::set_program_script(tx, ProgramTarget::Input(0), &Script::new())
            });
        let (program, script) = prepared_program();

        let prepared = builder
            .prepare_transaction(&program, &script, &Arguments::default(), &WitnessValues::default())
            .unwrap();

        assert!(hook_ran.get());
        assert!(prepared.inputs()[0].partial_input.witness_utxo.script_pubkey.is_empty());

        let failing_builder = FinalTransactionBuilder::new(initial_transaction(), [ProgramTarget::Input(0)])
            .unwrap()
            .with_post_hook(|_, _, _, _| Err(FuzzError::PostHook("hook failed".to_string())));
        let error = failing_builder
            .prepare_transaction(&program, &script, &Arguments::default(), &WitnessValues::default())
            .unwrap_err();

        assert_eq!(error, FuzzError::PostHook("hook failed".to_string()));
    }
}
