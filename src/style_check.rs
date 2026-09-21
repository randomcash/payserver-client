//! Shared plumbing for the "every class this file emits is defined in
//! `styles.css`" tests in `pages/plugin_page.rs` and `components/feedback.rs`.
//!
//! A class name in markup is a reference to a stylesheet rule, and a
//! reference that resolves to nothing is silent: the markup is valid and the
//! build stays green while the page renders unstyled. Kept as one copy
//! rather than two so the check cannot drift between the files that use it.

/// Whether `styles` has any rule that could match `class`.
///
/// Looks for the selector as a whole token - `.ps-card` must not be
/// satisfied by `.ps-card-body` - and accepts it anywhere a selector may
/// legally end: a brace, a comma, whitespace, a combinator, a pseudo, or
/// another class in a compound selector.
pub fn is_defined(styles: &str, class: &str) -> bool {
    let styles = strip_comments(styles);
    let needle = format!(".{class}");
    let mut from = 0;
    while let Some(at) = styles[from..].find(&needle) {
        let start = from + at;
        let after = styles[start + needle.len()..].chars().next();
        let before = styles[..start].chars().next_back();
        // Not preceded by an identifier character, or `.ps-page` would be
        // found inside a longer name it is not part of.
        let boundary_before =
            before.is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_');
        let boundary_after =
            after.is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_');
        if boundary_before && boundary_after {
            return true;
        }
        from = start + 1;
    }
    false
}

/// `/* ... */` removed, so a class named in prose is not mistaken for a
/// class that is styled.
pub fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(open) = rest.find("/*") {
        out.push_str(&rest[..open]);
        match rest[open + 2..].find("*/") {
            Some(close) => rest = &rest[open + 2 + close + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// The literal `class="..."` attributes in `source`, with `//` comment lines
/// dropped first since doc comments tend to quote class names while
/// explaining them.
///
/// This only sees classes written as a literal string. A class built as
/// `class={some_helper()}` is invisible to it - the same way it was invisible
/// to `rest.find("class=\"")` before this was extracted - so a caller with
/// such a helper must also call it and check its output, the way
/// `plugin_page.rs` does for `tone_class`, `notice_class` and `button_class`.
pub fn literal_classes(source: &str) -> Vec<String> {
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut classes = Vec::new();
    let mut rest = code.as_str();
    while let Some(at) = rest.find("class=\"") {
        rest = &rest[at + 7..];
        let Some(end) = rest.find('"') else { break };
        let (value, tail) = rest.split_at(end);
        rest = tail;
        classes.extend(value.split_whitespace().map(str::to_string));
    }
    classes
}
