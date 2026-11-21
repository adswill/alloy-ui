//! Compile-fail tests: every common macro mistake gets a clear message at the right spot.
//! Snapshots live next to the cases in `tests/ui/*.stderr`; regenerate them after changing a
//! message with `TRYBUILD=overwrite cargo test -p alloy-ui --test macro_errors`.

#[test]
fn macro_errors_are_clear() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
