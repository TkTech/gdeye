use crate::common::*;

#[test]
fn broken_node_path_dollar_syntax() {
    let output = run_gdeye("node_paths_project/player.gd");
    assert!(
        output.contains("NonExistentNode"),
        "Should flag $NonExistentNode as broken.\nOutput:\n{}",
        output
    );
}

#[test]
fn broken_node_path_get_node_call() {
    let output = run_gdeye("node_paths_project/player.gd");
    assert!(
        output.contains("DoesNotExist"),
        "Should flag get_node(\"DoesNotExist\") as broken.\nOutput:\n{}",
        output
    );
}

#[test]
fn broken_node_path_nested() {
    let output = run_gdeye("node_paths_project/player.gd");
    assert!(
        output.contains("Sprite/SubChild"),
        "Should flag $Sprite/SubChild as broken (Sprite has no SubChild).\nOutput:\n{}",
        output
    );
}

#[test]
fn broken_node_path_valid_not_flagged() {
    let output = run_gdeye("node_paths_project/player.gd");
    let broken_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("broken-node-path"))
        .collect();
    for line in &broken_lines {
        assert!(
            !line.contains("`Sprite`")
                && !line.contains("`Camera`")
                && !line.contains("`CollisionShape`"),
            "Should NOT flag valid node paths.\nBroken lines:\n{}",
            broken_lines.join("\n")
        );
    }
}

#[test]
fn broken_node_path_correct_count() {
    let output = run_gdeye("node_paths_project/player.gd");
    let count = output.matches("broken-node-path").count();
    assert_eq!(
        count, 5,
        "Should find exactly 5 broken node paths.\nOutput:\n{}",
        output
    );
}
