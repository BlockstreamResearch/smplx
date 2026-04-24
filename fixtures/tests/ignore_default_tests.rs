// Dummy test to insure that `simplex test` runs only `#[simplex::test]` functions

#[cfg(test)]
mod test {

    #[simplex::test]
    fn smplx_test_invoked(_: simplex::TestContext) -> anyhow::Result<()> {
        Ok(())
    }

    #[test]
    fn default_tests_should_not_be_invoked() {
        panic!()
    }
}
