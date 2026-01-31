use crate::config::IndentStyle;
use crate::fmt::ir::Doc;
use crate::fmt::FmtConfig;

/// Rendering mode for the current group context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// The group fits on one line; Line becomes space, Softline becomes empty.
    Flat,
    /// The group is broken; Line and Softline become newlines.
    Break,
}

/// A command on the printer stack: (indent_level, mode, doc_reference).
struct Cmd<'a> {
    indent: usize,
    mode: Mode,
    doc: &'a Doc,
}

/// Generate indentation string based on config.
fn make_indent(level: usize, config: &FmtConfig) -> String {
    match config.indent_style {
        IndentStyle::Tabs => "\t".repeat(level),
        IndentStyle::Spaces => " ".repeat(level * config.indent_size),
    }
}

/// Calculate the column width of an indentation level.
fn indent_width(level: usize, config: &FmtConfig) -> usize {
    match config.indent_style {
        IndentStyle::Tabs => level, // Each tab counts as 1 for width calculation
        IndentStyle::Spaces => level * config.indent_size,
    }
}

/// Format a Doc tree into a string, breaking lines to fit within `print_width`.
pub fn print(doc: &Doc, config: &FmtConfig) -> String {
    let mut out = String::new();
    let mut pos: usize = 0; // current column position
    let mut stack: Vec<Cmd> = vec![Cmd {
        indent: 0,
        mode: Mode::Break,
        doc,
    }];
    let mut line_suffixes: Vec<(usize, &Doc)> = Vec::new();

    while let Some(cmd) = stack.pop() {
        match cmd.doc {
            Doc::Text(s) => {
                out.push_str(s);
                pos += s.len();
            }
            Doc::Line => match cmd.mode {
                Mode::Flat => {
                    out.push(' ');
                    pos += 1;
                }
                Mode::Break => {
                    flush_line_suffixes(&mut out, &mut pos, &mut line_suffixes);
                    out.push('\n');
                    let indent_str = make_indent(cmd.indent, config);
                    out.push_str(&indent_str);
                    pos = indent_width(cmd.indent, config);
                }
            },
            Doc::Softline => match cmd.mode {
                Mode::Flat => {
                    // empty — no space, no newline
                }
                Mode::Break => {
                    flush_line_suffixes(&mut out, &mut pos, &mut line_suffixes);
                    out.push('\n');
                    let indent_str = make_indent(cmd.indent, config);
                    out.push_str(&indent_str);
                    pos = indent_width(cmd.indent, config);
                }
            },
            Doc::Hardline => {
                flush_line_suffixes(&mut out, &mut pos, &mut line_suffixes);
                out.push('\n');
                let indent_str = make_indent(cmd.indent, config);
                out.push_str(&indent_str);
                pos = indent_width(cmd.indent, config);
            }
            Doc::Indent(inner) => {
                stack.push(Cmd {
                    indent: cmd.indent + 1,
                    mode: cmd.mode,
                    doc: inner,
                });
            }
            Doc::Concat(docs) => {
                // Push in reverse so the first doc is processed first.
                for d in docs.iter().rev() {
                    stack.push(Cmd {
                        indent: cmd.indent,
                        mode: cmd.mode,
                        doc: d,
                    });
                }
            }
            Doc::Group(inner) => {
                let mode = if fits(inner, config.print_width.saturating_sub(pos), cmd.indent) {
                    Mode::Flat
                } else {
                    Mode::Break
                };
                stack.push(Cmd {
                    indent: cmd.indent,
                    mode,
                    doc: inner,
                });
            }
            Doc::IfBreak { broken, flat } => {
                let chosen = match cmd.mode {
                    Mode::Flat => flat,
                    Mode::Break => broken,
                };
                stack.push(Cmd {
                    indent: cmd.indent,
                    mode: cmd.mode,
                    doc: chosen,
                });
            }
            Doc::LineSuffix(inner) => {
                line_suffixes.push((cmd.indent, inner));
            }
            Doc::BreakParent => {
                // BreakParent is handled during fits() check; it forces the
                // enclosing group to break. At print time it's a no-op.
            }
        }
    }

    // Flush any remaining line suffixes at the end of the document.
    flush_line_suffixes(&mut out, &mut pos, &mut line_suffixes);
    out
}

/// Flush buffered line suffixes (trailing comments) before a newline.
fn flush_line_suffixes(out: &mut String, pos: &mut usize, suffixes: &mut Vec<(usize, &Doc)>) {
    if suffixes.is_empty() {
        return;
    }
    let taken: Vec<(usize, &Doc)> = std::mem::take(suffixes);
    for (_indent, doc) in taken {
        // Trailing comments are rendered flat (they should fit on the line).
        render_flat(out, pos, doc);
    }
}

/// Render a doc in flat mode (no line breaks). Used for line suffixes.
fn render_flat(out: &mut String, pos: &mut usize, doc: &Doc) {
    match doc {
        Doc::Text(s) => {
            out.push_str(s);
            *pos += s.len();
        }
        Doc::Line => {
            out.push(' ');
            *pos += 1;
        }
        Doc::Softline => {}
        Doc::Hardline => {
            // Shouldn't appear in a line suffix, but handle gracefully.
            out.push('\n');
            *pos = 0;
        }
        Doc::Indent(inner) => render_flat(out, pos, inner),
        Doc::Concat(docs) => {
            for d in docs {
                render_flat(out, pos, d);
            }
        }
        Doc::Group(inner) => render_flat(out, pos, inner),
        Doc::IfBreak { flat, .. } => render_flat(out, pos, flat),
        Doc::LineSuffix(inner) => render_flat(out, pos, inner),
        Doc::BreakParent => {}
    }
}

/// Check whether `doc` can be rendered flat within `remaining` columns.
fn fits(doc: &Doc, remaining: usize, indent: usize) -> bool {
    let mut budget = remaining as isize;
    let mut stack: Vec<(&Doc, usize)> = vec![(doc, indent)];

    while let Some((d, ind)) = stack.pop() {
        if budget < 0 {
            return false;
        }
        match d {
            Doc::Text(s) => {
                budget -= s.len() as isize;
            }
            Doc::Line => {
                // In flat mode, Line becomes a space.
                budget -= 1;
            }
            Doc::Softline => {
                // In flat mode, Softline is empty.
            }
            Doc::Hardline => {
                // Hardline forces a break — group cannot be flat.
                return false;
            }
            Doc::Indent(inner) => {
                stack.push((inner, ind + 1));
            }
            Doc::Concat(docs) => {
                for d in docs.iter().rev() {
                    stack.push((d, ind));
                }
            }
            Doc::Group(inner) => {
                // Nested groups: try flat as well.
                stack.push((inner, ind));
            }
            Doc::IfBreak { flat, .. } => {
                // When checking flat mode, use the flat branch.
                stack.push((flat, ind));
            }
            Doc::LineSuffix(_) => {
                // Line suffixes don't affect width during fits check.
            }
            Doc::BreakParent => {
                // BreakParent forces the enclosing group to break.
                return false;
            }
        }
    }
    budget >= 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fmt::ir::*;

    fn config_with_width(width: usize) -> FmtConfig {
        FmtConfig {
            print_width: width,
            ..Default::default()
        }
    }

    #[test]
    fn simple_text() {
        let doc = text("hello");
        assert_eq!(print(&doc, &config_with_width(80)), "hello");
    }

    #[test]
    fn concat_texts() {
        let doc = concat(vec![text("hello"), text(" "), text("world")]);
        assert_eq!(print(&doc, &config_with_width(80)), "hello world");
    }

    #[test]
    fn group_fits_on_line() {
        let doc = group(concat(vec![text("a"), line(), text("b")]));
        assert_eq!(print(&doc, &config_with_width(80)), "a b");
    }

    #[test]
    fn group_breaks_when_too_long() {
        let doc = group(concat(vec![text("a"), line(), text("b")]));
        // Width of 2 means "a b" (3 chars) doesn't fit.
        assert_eq!(print(&doc, &config_with_width(2)), "a\nb");
    }

    #[test]
    fn indent_adds_tab() {
        let doc = concat(vec![
            text("if true:"),
            indent(concat(vec![hardline(), text("pass")])),
        ]);
        assert_eq!(print(&doc, &config_with_width(80)), "if true:\n\tpass");
    }

    #[test]
    fn nested_indent() {
        let doc = concat(vec![
            text("a"),
            indent(concat(vec![
                hardline(),
                text("b"),
                indent(concat(vec![hardline(), text("c")])),
            ])),
        ]);
        assert_eq!(print(&doc, &config_with_width(80)), "a\n\tb\n\t\tc");
    }

    #[test]
    fn softline_empty_when_flat() {
        let doc = group(concat(vec![text("a"), softline(), text("b")]));
        assert_eq!(print(&doc, &config_with_width(80)), "ab");
    }

    #[test]
    fn softline_newline_when_broken() {
        let doc = group(concat(vec![text("a"), softline(), text("b")]));
        assert_eq!(print(&doc, &config_with_width(1)), "a\nb");
    }

    #[test]
    fn hardline_always_breaks() {
        let doc = group(concat(vec![text("a"), hardline(), text("b")]));
        assert_eq!(print(&doc, &config_with_width(80)), "a\nb");
    }

    #[test]
    fn if_break_flat() {
        let doc = group(concat(vec![
            text("("),
            if_break(text(","), text("")),
            text(")"),
        ]));
        assert_eq!(print(&doc, &config_with_width(80)), "()");
    }

    #[test]
    fn if_break_broken() {
        let doc = group(concat(vec![
            text("("),
            line(),
            text("very_long_content_here"),
            if_break(text(","), text("")),
            line(),
            text(")"),
        ]));
        assert_eq!(
            print(&doc, &config_with_width(10)),
            "(\nvery_long_content_here,\n)"
        );
    }

    #[test]
    fn line_suffix_trailing_comment() {
        let doc = concat(vec![
            text("var x = 1"),
            line_suffix(text(" # comment")),
            hardline(),
            text("var y = 2"),
        ]);
        assert_eq!(
            print(&doc, &config_with_width(80)),
            "var x = 1 # comment\nvar y = 2"
        );
    }

    #[test]
    fn break_parent_forces_break() {
        let doc = group(concat(vec![text("a"), Doc::BreakParent, line(), text("b")]));
        assert_eq!(print(&doc, &config_with_width(80)), "a\nb");
    }

    #[test]
    fn indent_with_spaces() {
        let config = FmtConfig {
            print_width: 80,
            indent_style: IndentStyle::Spaces,
            indent_size: 4,
            ..Default::default()
        };
        let doc = concat(vec![
            text("if true:"),
            indent(concat(vec![hardline(), text("pass")])),
        ]);
        assert_eq!(print(&doc, &config), "if true:\n    pass");
    }

    #[test]
    fn indent_with_2_spaces() {
        let config = FmtConfig {
            print_width: 80,
            indent_style: IndentStyle::Spaces,
            indent_size: 2,
            ..Default::default()
        };
        let doc = concat(vec![
            text("a"),
            indent(concat(vec![
                hardline(),
                text("b"),
                indent(concat(vec![hardline(), text("c")])),
            ])),
        ]);
        assert_eq!(print(&doc, &config), "a\n  b\n    c");
    }

    #[test]
    fn join_with_line() {
        let items = vec![text("a"), text("b"), text("c")];
        let doc = group(join(items, line()));
        assert_eq!(print(&doc, &config_with_width(80)), "a b c");
        assert_eq!(print(&doc, &config_with_width(3)), "a\nb\nc");
    }
}
