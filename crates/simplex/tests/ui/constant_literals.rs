use simplex::include_simf;
use simplex::program::Program;
use simplex::simplicityhl::Arguments;

pub struct ConstantLiteralsProgram {
    program: Program,
}

impl ConstantLiteralsProgram {
    pub const SOURCE: &'static str = derived_constant_literals::CONSTANT_LITERALS_CONTRACT_SOURCE;

    pub fn new(arguments: impl Into<Arguments>) -> Self {
        Self {
            program: Program::new(Self::SOURCE, arguments.into()),
        }
    }
}

impl AsRef<Program> for ConstantLiteralsProgram {
    fn as_ref(&self) -> &Program {
        &self.program
    }
}

include_simf!("../../../../crates/simplex/tests/ui_simfs/constant_literals.simf");

fn main() -> Result<(), String> {
    let _ = test_constants()?;

    Ok(())
}

fn test_constants() -> Result<(), String> {
    use simplex::simplicityhl::Value;
    use simplex::simplicityhl::num::U256;
    use simplex::simplicityhl::value::ValueConstructible;
    use std::collections::HashSet;

    let constants = ConstantLiteralsProgram::get_constants();
    let expected: HashSet<_> = [
        Value::u1(1),
        Value::u2(3),
        Value::u4(15),
        Value::u8(u8::MAX),
        Value::u16(u16::MAX),
        Value::u32(u32::MAX),
        Value::u64(u64::MAX),
        Value::u128(u128::MAX),
        Value::u256(U256::from_byte_array([255; 32])),
        Value::byte_array([0, 1, 2]),
        Value::u16(1337),
        Value::u32(1337),
        Value::u8(7),
        Value::u16(8),
        Value::u16(9),
        Value::from(true),
        Value::from(false),
        Value::u8(1),
        Value::u8(2),
        Value::u8(0),
        Value::u8(42),
        Value::u16(5),
        Value::u8(6),
        Value::u16(6),
    ]
    .into_iter()
    .collect();
    let actual: HashSet<_> = constants
        .iter()
        .map(|(ty, value)| {
            assert_eq!(ty, value.ty());
            value.clone()
        })
        .collect();

    assert_eq!(actual, expected);
    assert_eq!(constants.len(), actual.len(), "constants must be unique");
    assert!(std::ptr::eq(constants, ConstantLiteralsProgram::get_constants()));

    Ok(())
}
