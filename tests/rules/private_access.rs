use crate::common::*;

#[test]
fn self_private_access_ok() {
    let output = run_gdeye("correctness_private_access.gd");
    // Accessing own private members should not be flagged
    let count = count_rule(&output, "correctness/private-access");
    assert_eq!(
        count, 0,
        "Should not flag access to own private members. Got {}.\nOutput:\n{}",
        count, output
    );
}
