/// Document IR for the Wadler-Lindig pretty-printer.
///
/// Each variant represents a formatting instruction that the layout algorithm
/// interprets to produce the final output string.
#[derive(Debug, Clone)]
pub enum Doc {
    /// Literal text (never broken across lines).
    Text(String),
    /// A potential line break: renders as a space when the enclosing group fits
    /// on one line (flat mode), or as a newline when the group is broken.
    Line,
    /// A potential line break: renders as empty when flat, or as a newline when broken.
    Softline,
    /// An unconditional line break (always renders as a newline).
    Hardline,
    /// Increases indentation by one level for the contained document.
    Indent(Box<Doc>),
    /// A sequence of documents rendered in order.
    Concat(Vec<Doc>),
    /// Attempts to render the contained document flat (on one line). If it
    /// doesn't fit within the remaining width, the group is broken and all
    /// Line/Softline nodes inside render as newlines.
    Group(Box<Doc>),
    /// Renders `flat` when the enclosing group is flat, or `broken` when the
    /// enclosing group is broken.
    IfBreak { broken: Box<Doc>, flat: Box<Doc> },
    /// Defers the contained document to the end of the current line. Used for
    /// trailing comments so they appear after the code on the same line.
    LineSuffix(Box<Doc>),
    /// Forces the enclosing group to break (used when content contains a
    /// Hardline or otherwise cannot fit on one line).
    BreakParent,
}

/// Create a Text document from a string.
pub fn text<S: Into<String>>(s: S) -> Doc {
    Doc::Text(s.into())
}

/// A line break that becomes a space when flat.
pub fn line() -> Doc {
    Doc::Line
}

/// A line break that becomes empty when flat.
pub fn softline() -> Doc {
    Doc::Softline
}

/// An unconditional line break.
pub fn hardline() -> Doc {
    Doc::Hardline
}

/// Indent the contained document by one level.
pub fn indent(doc: Doc) -> Doc {
    Doc::Indent(Box::new(doc))
}

/// Group: try to render flat, break if it doesn't fit.
pub fn group(doc: Doc) -> Doc {
    Doc::Group(Box::new(doc))
}

/// Concatenate a sequence of documents.
pub fn concat(docs: Vec<Doc>) -> Doc {
    Doc::Concat(docs)
}

/// Conditional rendering based on whether the enclosing group is broken.
pub fn if_break(broken: Doc, flat: Doc) -> Doc {
    Doc::IfBreak {
        broken: Box::new(broken),
        flat: Box::new(flat),
    }
}

/// Trailing content that is deferred to the end of the current line.
pub fn line_suffix(doc: Doc) -> Doc {
    Doc::LineSuffix(Box::new(doc))
}

/// Force the enclosing group to break.
pub fn break_parent() -> Doc {
    Doc::BreakParent
}

/// Join a list of documents with a separator between each pair.
pub fn join(docs: Vec<Doc>, separator: Doc) -> Doc {
    let mut result = Vec::new();
    let len = docs.len();
    for (i, doc) in docs.into_iter().enumerate() {
        result.push(doc);
        if i + 1 < len {
            result.push(separator.clone());
        }
    }
    Doc::Concat(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_creates_text_doc() {
        match text("hello") {
            Doc::Text(s) => assert_eq!(s, "hello"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn join_empty() {
        match join(vec![], line()) {
            Doc::Concat(docs) => assert!(docs.is_empty()),
            _ => panic!("expected Concat"),
        }
    }

    #[test]
    fn join_single() {
        match join(vec![text("a")], line()) {
            Doc::Concat(docs) => assert_eq!(docs.len(), 1),
            _ => panic!("expected Concat"),
        }
    }

    #[test]
    fn join_multiple() {
        match join(vec![text("a"), text("b"), text("c")], line()) {
            Doc::Concat(docs) => assert_eq!(docs.len(), 5), // a, line, b, line, c
            _ => panic!("expected Concat"),
        }
    }
}
