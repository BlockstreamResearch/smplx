use simplex::alias::{Asset1, ExplicitAsset, ExplicitNonce, Lock, Message, Nonce, Point, Pubkey};
use simplex::program::Program;
use simplex::simplicityhl::elements::secp256k1_zkp::{Parity, constants};
use simplex::simplicityhl::{Arguments, WitnessValues};

#[derive(Clone)]
pub struct SafeAliasesProgram(Program);

impl SafeAliasesProgram {
    #[must_use]
    pub fn new(arguments: impl Into<Arguments>) -> Self {
        Self(Program::new(
            derived_safe_aliases::SAFE_ALIASES_CONTRACT_SOURCE,
            arguments,
        ))
    }
}

impl AsRef<Program> for SafeAliasesProgram {
    fn as_ref(&self) -> &Program {
        &self.0
    }
}

simplex::include_simf!("simf/safe_aliases.simf");

fn main() -> anyhow::Result<()> {
    use derived_safe_aliases::{
        SafeAliasesArguments, SafeAliasesArgumentsSafe, SafeAliasesWitness, SafeAliasesWitnessSafe,
    };

    let arguments = SafeAliasesArgumentsSafe { lock: Lock(42) };
    let ordinary_arguments: SafeAliasesArguments = (&arguments).into();
    assert_eq!(ordinary_arguments.lock, 42);

    let program = SafeAliasesProgram::new(arguments);

    let pubkey = Pubkey::from_bytes(constants::GENERATOR_X)?;
    let point = Point::from_pubkey(pubkey, Parity::Even)?;
    let safe = SafeAliasesWitnessSafe {
        asset: Asset1::from_explicit(ExplicitAsset([0x42; 32])),
        nonce: Nonce::from_explicit(ExplicitNonce([0x55; 32])),
        payload: (Some(Message([0x11; 32])), [point; 2]),
        count: 7,
    };

    // Conversion uses SimplicityEncode for aliases, including nested fields.
    let ordinary: SafeAliasesWitness = (&safe).into();
    assert_eq!(ordinary.count, 7);
    assert_eq!(ordinary.payload.0, Some([0x11; 32]));
    assert_eq!(ordinary.payload.1, [(0, constants::GENERATOR_X); 2]);
    assert_eq!(WitnessValues::from(&safe), WitnessValues::from(&ordinary));

    let moved: SafeAliasesWitness = safe.into();
    assert_eq!(moved, ordinary);

    program
        .as_ref()
        .load()?
        .satisfy(ordinary.into())
        .map_err(anyhow::Error::msg)?;

    Ok(())
}
