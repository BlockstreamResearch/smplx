// Checks that `simplex test` runs only `#[simplex::test]` functions

#[cfg(test)]
mod test {
    #[simplex::test]
    fn simplex_test_is_run(_: simplex::TestContext) -> anyhow::Result<()> {
        Ok(())
    }

    #[test]
    fn plain_test_is_not_run() {
        panic!()
    }
}
