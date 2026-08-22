//! Port of `entrec.py --selftest`, as a test.

use tmtraj::testonly::selftest;

#[test]
fn selftest_passes() {
    let r = selftest(true);
    println!(
        "\n{} checks, {} failures, {} cases skipped",
        r.checks,
        r.failures.len(),
        r.skipped.len()
    );
    assert!(r.skipped.is_empty(), "reference ghost missing: {:?}", r.skipped);
    assert!(r.ok, "failures: {:?}", r.failures);
    assert!(r.checks >= 22, "expected 22+ checks, got {}", r.checks);
}
