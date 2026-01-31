use crate::common::*;

#[test]
fn cross_file_used_function_not_flagged() {
    let output = run_gdeye_with_args(
        "cross_file_usage",
        &[
            "--disable",
            "style/naming-convention",
            "--disable",
            "style/untyped-parameter",
            "--disable",
            "style/untyped-return",
            "--disable",
            "style/function-too-long",
            "--disable",
            "style/excessive-nesting",
        ],
    );
    // take_damage is called via player.take_damage() in game.gd
    let unused_func_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("unused-function"))
        .collect();
    for line in &unused_func_lines {
        assert!(
            !line.contains("`take_damage`"),
            "Should NOT flag `take_damage` -- called cross-file.\nLines:\n{}",
            unused_func_lines.join("\n")
        );
    }
}

#[test]
fn cross_file_scene_handler_marks_function_used() {
    let output = run_gdeye_with_args(
        "cross_file_usage",
        &[
            "--disable",
            "style/naming-convention",
            "--disable",
            "style/untyped-parameter",
            "--disable",
            "style/untyped-return",
            "--disable",
            "style/function-too-long",
            "--disable",
            "style/excessive-nesting",
        ],
    );
    // _game_over is connected as signal handler in main.tscn
    let unused_func_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("unused-function"))
        .collect();
    for line in &unused_func_lines {
        assert!(
            !line.contains("`_game_over`"),
            "Should NOT flag `_game_over` -- used as signal handler in scene.\nLines:\n{}",
            unused_func_lines.join("\n")
        );
    }
}
