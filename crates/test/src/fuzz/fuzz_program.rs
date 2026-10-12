use simplicityhl::elements::Script;
use simplicityhl::{Arguments, WitnessValues};

use smplx_sdk::program::{Program, ProgramFactory, ProgramSchema};
use smplx_sdk::provider::SimplicityNetwork;

/// A program the fuzz engine can build, describe and decode. Implemented for every `include_simf!()` program.
pub trait FuzzableProgram:
    AsRef<Program>
    + ProgramFactory<Self>
    + ProgramSchema<
        Arguments: for<'a> TryFrom<&'a Arguments, Error = String>,
        Witness: for<'a> TryFrom<&'a WitnessValues, Error = String>,
    > + Clone
    + 'static
{
    fn build_program(args: impl Into<Arguments>, network: &SimplicityNetwork) -> (Box<Self>, Script) {
        let program = Self::instantiate_program(args);
        let script = (*program).as_ref().get_script_pubkey(network);

        (program, script)
    }
}

impl<P> FuzzableProgram for P where
    P: AsRef<Program>
        + ProgramFactory<P>
        + ProgramSchema<
            Arguments: for<'a> TryFrom<&'a Arguments, Error = String>,
            Witness: for<'a> TryFrom<&'a WitnessValues, Error = String>,
        > + Clone
        + 'static
{
}
