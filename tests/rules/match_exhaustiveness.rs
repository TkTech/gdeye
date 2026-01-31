use crate::common::{count_rule, run_gdeye};

#[test]
fn match_exhaustiveness_missing_variant() {
    let output = run_gdeye("match_exhaustiveness.gd");
    assert!(
        count_rule(&output, "match-exhaustiveness") >= 1,
        "Should warn about non-exhaustive match.\nOutput:\n{}",
        output
    );
    assert!(
        output.contains("FALLING"),
        "Should mention missing FALLING variant.\nOutput:\n{}",
        output
    );
}

#[test]
fn match_exhaustiveness_complete_not_flagged() {
    let output = run_gdeye("match_exhaustiveness.gd");
    // The complete match (check_state_complete) should not be flagged
    // We verify by checking only 1 warning exists (for the incomplete match)
    assert_eq!(
        count_rule(&output, "match-exhaustiveness"),
        1,
        "Only one match should be flagged as non-exhaustive.\nOutput:\n{}",
        output
    );
}
