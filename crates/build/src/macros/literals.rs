use simplicityhl::lexer::{Token, lex};
use simplicityhl::num::U256;

/// Sorted, deduplicated integer literals of `source`.
pub fn extract_literals(source: &str) -> Vec<U256> {
    let (tokens, _) = lex(0, source, 0);

    let mut literals: Vec<U256> = tokens
        .into_iter()
        .flatten()
        .filter_map(|(token, _)| match token {
            Token::DecLiteral(digits) => parse_digits(digits.as_inner(), 10),
            Token::HexLiteral(digits) => parse_digits(digits.as_inner(), 16),
            Token::BinLiteral(digits) => parse_digits(digits.as_inner(), 2),
            _ => None,
        })
        .collect();

    literals.sort_unstable();
    literals.dedup();

    literals
}

fn parse_digits(digits: &str, radix: u32) -> Option<U256> {
    let mut bytes = [0u8; 32];

    for digit in digits.chars().filter(|c| *c != '_') {
        let mut carry = digit.to_digit(radix)?;

        for byte in bytes.iter_mut().rev() {
            let value = u32::from(*byte) * radix + carry;
            *byte = (value % 256) as u8;
            carry = value / 256;
        }

        if carry != 0 {
            return None;
        }
    }

    Some(U256::from_byte_array(bytes))
}

#[cfg(test)]
mod tests {
    use simplicityhl::num::U256;

    use super::extract_literals;

    fn u256(value: u128) -> U256 {
        U256::from(value)
    }

    #[test]
    fn extracts_decimal_hex_and_binary_literals() {
        let source = "fn main() { let a: u16 = 1337; let b: u8 = 0xff; let c: u4 = 0b101; let d: u32 = 10_000; }";

        assert_eq!(
            extract_literals(source),
            vec![u256(5), u256(255), u256(1337), u256(10_000)]
        );
    }

    #[test]
    fn skips_comments_and_deduplicates() {
        let source = "// 42\nfn main() { /* 7 */ let a: u8 = 3; let b: u8 = 3; }";

        assert_eq!(extract_literals(source), vec![u256(3)]);
    }

    #[test]
    fn handles_a_leading_version_directive() {
        let source = "simc \">=0.1\";\nfn main() { let a: u16 = 1337; }";

        assert!(extract_literals(source).contains(&u256(1337)));
    }

    #[test]
    fn keeps_full_width_u256_literals() {
        let source = format!("fn main() {{ let a: u256 = 0x{}; }}", "f".repeat(64));

        assert_eq!(extract_literals(&source), vec![U256::from_byte_array([0xff; 32])]);
    }
}
