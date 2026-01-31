use crate::common::*;

#[test]
fn unreachable_after_return() {
    let output = run_gdeye("correctness_unreachable_code.gd");
    assert!(
        has_message(&output, "Unreachable code"),
        "Should detect unreachable code after return.\nOutput:\n{}",
        output
    );
}

#[test]
fn unreachable_correct_count() {
    let output = run_gdeye("correctness_unreachable_code.gd");
    let count = count_rule(&output, "correctness/unreachable-code");
    assert_eq!(
        count, 4,
        "Expected 4 unreachable-code warnings (after return, break, continue, nested return). Got {}.\nOutput:\n{}",
        count, output
    );
}

#[test]
fn unreachable_not_flagged_conditional_return() {
    let output = run_gdeye_stdout("correctness_unreachable_code.gd", &["--format", "json"]);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let results = parsed.as_array().unwrap();
    for r in results {
        let line = r["line"].as_u64().unwrap();
        assert!(
            !(24..=28).contains(&line),
            "Should not flag conditional_return function. Line: {}",
            line
        );
        assert!(
            !(31..=35).contains(&line),
            "Should not flag conditional_break function. Line: {}",
            line
        );
        assert!(
            !(38..=41).contains(&line),
            "Should not flag normal function. Line: {}",
            line
        );
    }
}
