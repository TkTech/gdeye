use crate::common::*;

#[test]
fn style_onready_hoist_simple_detected() {
    let output = run_gdeye("style_onready_hoist.gd");
    assert!(
        has_message(&output, "onready-hoist") && has_message(&output, "label"),
        "Should detect member var with $Node missing @onready.\nOutput:\n{}",
        output
    );
}

#[test]
fn style_onready_hoist_nested_path_detected() {
    let output = run_gdeye("style_onready_hoist.gd");
    assert!(
        has_message(&output, "onready-hoist") && has_message(&output, "button"),
        "Should detect member var with nested $UI/Button path.\nOutput:\n{}",
        output
    );
}

#[test]
fn style_onready_hoist_existing_onready_not_flagged() {
    let output = run_gdeye("style_onready_hoist.gd");
    assert!(
        !output.contains("sprite")
            || !output
                .lines()
                .any(|l| l.contains("onready-hoist") && l.contains("sprite")),
        "Should NOT flag variable that already has @onready.\nOutput:\n{}",
        output
    );
}

#[test]
fn style_onready_hoist_non_node_path_not_flagged() {
    let output = run_gdeye("style_onready_hoist.gd");
    let onready_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("onready-hoist"))
        .collect();

    for line in &onready_lines {
        assert!(
            !line.contains("counter") && !line.contains("name_str"),
            "Should NOT flag member vars without node path.\nLine: {}\nOutput:\n{}",
            line,
            output
        );
    }
}

#[test]
fn style_onready_hoist_local_var_not_flagged() {
    let output = run_gdeye("style_onready_hoist.gd");
    assert!(
        !output.contains("local_label"),
        "Should NOT flag local variables inside functions.\nOutput:\n{}",
        output
    );
}

#[test]
fn style_onready_hoist_ready_assignment_detected() {
    let output = run_gdeye("style_onready_hoist.gd");
    assert!(
        has_message(&output, "onready-hoist")
            && has_message(&output, "player")
            && has_message(&output, "hoisted"),
        "Should detect member var assigned in _ready() that can be hoisted.\nOutput:\n{}",
        output
    );
}

#[test]
fn style_onready_hoist_ready_assignment_typed_detected() {
    let output = run_gdeye("style_onready_hoist.gd");
    assert!(
        has_message(&output, "onready-hoist")
            && has_message(&output, "enemy")
            && has_message(&output, "hoisted"),
        "Should detect typed member var assigned in _ready() that can be hoisted.\nOutput:\n{}",
        output
    );
}

#[test]
fn style_onready_hoist_non_node_assignment_not_flagged() {
    let output = run_gdeye("style_onready_hoist.gd");
    let onready_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("onready-hoist"))
        .collect();

    for line in &onready_lines {
        assert!(
            !line.contains("health"),
            "Should NOT flag member vars assigned non-node-path in _ready().\nLine: {}\nOutput:\n{}",
            line,
            output
        );
    }
}

#[test]
fn style_onready_hoist_has_fix() {
    let output = run_gdeye("style_onready_hoist.gd");
    // The rule provides fixes - count should match diagnostics
    let count = count_rule(&output, "style/onready-hoist");
    assert!(
        count >= 4,
        "Expected at least 4 onready-hoist warnings (label, button, player hoist, enemy hoist). Got {}.\nOutput:\n{}",
        count,
        output
    );
}

#[test]
fn style_onready_hoist_inner_class_detected() {
    let output = run_gdeye("style_onready_hoist.gd");
    assert!(
        has_message(&output, "onready-hoist") && has_message(&output, "inner_label"),
        "Should detect member var in inner class missing @onready.\nOutput:\n{}",
        output
    );
}
