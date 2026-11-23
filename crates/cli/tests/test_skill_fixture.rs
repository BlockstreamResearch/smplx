#[path = "../../../skills/assets/contract_test_fixture.rs"]
mod fixture;

#[test]
fn bundled_fixture_executes_real_signatures_and_rejects_transaction_changes() {
    let source = "fn main() { jet::bip_0340_verify((param::OWNER, jet::sig_all_hash()), witness::AUTH) }";
    let args = [("OWNER", "@PK1")];
    let witness = [("AUTH", "@SIG1")];
    assert!(fixture::evaluate(source, &args, &witness, fixture::Tx::default()).is_ok());
    let failure = fixture::evaluate(source, &args, &[("AUTH", "@SIG2")], fixture::Tx::default()).unwrap_err();
    assert_eq!(failure.stage, "execution", "{failure:?}");
    let failure = fixture::evaluate(
        source,
        &args,
        &witness,
        fixture::Tx {
            mutate_output_after_signing: true,
            ..fixture::Tx::default()
        },
    )
    .unwrap_err();
    assert_eq!(failure.stage, "execution", "{failure:?}");
}
