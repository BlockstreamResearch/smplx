#[cfg(test)]
mod tests {
    use simplex_sdk::constants::DUMMY_SIGNATURE;
    use simplex_sdk::presets::p2pk::p2pk_build::P2PKWitness;
    use simplex_sdk::signer::Signer;

    #[test]
    #[ignore]
    fn main() {
        let signer = Signer::from_seed(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .as_bytes()
                .try_into()
                .unwrap(),
        )
        .unwrap();

        let witness = P2PKWitness {
            signature: DUMMY_SIGNATURE,
        };

        // let arguments = P2PKArguments {
        //     public_key: signer.public_key(),
        // };

        // let p2pk = P2PK::new(&tr_unspendable_key(), &arguments);
    }
}
