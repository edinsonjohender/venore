//! String manipulation utilities

use once_cell::sync::Lazy;
use regex::Regex;

/// Regex matching ANSI escape sequences: SGR, cursor, erase, OSC, and DEC private modes.
static ANSI_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\x1b\[[0-9;]*[mHJKG]|\x1b\].*?\x07|\x1b\[[\?0-9;]*[hlr]").unwrap()
});

/// Strip ANSI escape sequences from a string.
///
/// Removes SGR (color/style), cursor movement, erase, OSC, and DEC private mode
/// sequences. Used to clean terminal output before feeding it to the AI buffer.
///
/// # Examples
/// ```
/// use venore_core::utils::strip_ansi_escapes;
///
/// assert_eq!(strip_ansi_escapes("\x1b[32mOK\x1b[0m"), "OK");
/// assert_eq!(strip_ansi_escapes("plain text"), "plain text");
/// ```
pub fn strip_ansi_escapes(s: &str) -> String {
    ANSI_RE.replace_all(s, "").to_string()
}

/// Capitalize first letter
///
/// # Examples
/// ```
/// use venore_core::utils::capitalize;
///
/// assert_eq!(capitalize("hello"), "Hello");
/// assert_eq!(capitalize(""), "");
/// assert_eq!(capitalize("HELLO"), "HELLO");
/// ```
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().chain(chars).collect(),
    }
}

/// Convert kebab-case to Title Case
///
/// # Examples
/// ```
/// use venore_core::utils::kebab_to_title;
///
/// assert_eq!(kebab_to_title("hello-world"), "Hello World");
/// assert_eq!(kebab_to_title("my-awesome-project"), "My Awesome Project");
/// ```
pub fn kebab_to_title(s: &str) -> String {
    s.split('-')
        .map(capitalize)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Convert snake_case to Title Case
///
/// # Examples
/// ```
/// use venore_core::utils::snake_to_title;
///
/// assert_eq!(snake_to_title("hello_world"), "Hello World");
/// assert_eq!(snake_to_title("my_awesome_function"), "My Awesome Function");
/// ```
pub fn snake_to_title(s: &str) -> String {
    s.split('_')
        .map(capitalize)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Convert camelCase to Title Case
///
/// # Examples
/// ```
/// use venore_core::utils::camel_to_title;
///
/// assert_eq!(camel_to_title("helloWorld"), "Hello World");
/// assert_eq!(camel_to_title("myAwesomeFunction"), "My Awesome Function");
/// ```
pub fn camel_to_title(s: &str) -> String {
    let mut result = String::new();
    let mut prev_was_lowercase = false;

    for c in s.chars() {
        if c.is_uppercase() && prev_was_lowercase {
            result.push(' ');
        }
        result.push(c);
        prev_was_lowercase = c.is_lowercase();
    }

    capitalize(&result)
}

/// Sanitize filename (remove invalid characters)
///
/// # Examples
/// ```
/// use venore_core::utils::sanitize_filename;
///
/// assert_eq!(sanitize_filename("file/name?.txt"), "file_name_.txt");
/// assert_eq!(sanitize_filename("valid_name.rs"), "valid_name.rs");
/// ```
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

/// Truncate string with ellipsis
///
/// # Examples
/// ```
/// use venore_core::utils::truncate;
///
/// assert_eq!(truncate("Hello World", 8, "..."), "Hello...");
/// assert_eq!(truncate("Short", 10, "..."), "Short");
/// ```
pub fn truncate(s: &str, max_length: usize, ellipsis: &str) -> String {
    if s.len() <= max_length {
        s.to_string()
    } else {
        let truncate_at = max_length.saturating_sub(ellipsis.len());
        format!("{}{}", &s[..s.floor_char_boundary(truncate_at)], ellipsis)
    }
}

/// Truncate middle of string
///
/// # Examples
/// ```
/// use venore_core::utils::truncate_middle;
///
/// assert_eq!(truncate_middle("HelloWorld", 8), "He...ld");
/// assert_eq!(truncate_middle("Short", 10), "Short");
/// ```
pub fn truncate_middle(s: &str, max_length: usize) -> String {
    if s.len() <= max_length {
        return s.to_string();
    }

    let ellipsis = "...";
    let side_length = (max_length.saturating_sub(ellipsis.len())) / 2;

    let left = s.floor_char_boundary(side_length);
    let right = s.ceil_char_boundary(s.len().saturating_sub(side_length));

    format!("{}{}{}", &s[..left], ellipsis, &s[right..])
}

/// Normalize whitespace (multiple spaces → single space)
///
/// # Examples
/// ```
/// use venore_core::utils::normalize_whitespace;
///
/// assert_eq!(normalize_whitespace("hello    world"), "hello world");
/// assert_eq!(normalize_whitespace("  trim  me  "), "trim me");
/// ```
pub fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Count words
///
/// # Examples
/// ```
/// use venore_core::utils::word_count;
///
/// assert_eq!(word_count("hello world"), 2);
/// assert_eq!(word_count("one"), 1);
/// assert_eq!(word_count(""), 0);
/// ```
pub fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

/// Simple pluralization
///
/// # Examples
/// ```
/// use venore_core::utils::pluralize;
///
/// assert_eq!(pluralize(1, "file", None), "1 file");
/// assert_eq!(pluralize(2, "file", None), "2 files");
/// assert_eq!(pluralize(2, "child", Some("children")), "2 children");
/// ```
pub fn pluralize(count: usize, singular: &str, plural: Option<&str>) -> String {
    if count == 1 {
        format!("{} {}", count, singular)
    } else {
        match plural {
            Some(p) => format!("{} {}", count, p),
            None => format!("{} {}s", count, singular),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi_escapes() {
        assert_eq!(strip_ansi_escapes("\x1b[32mOK\x1b[0m"), "OK");
        assert_eq!(strip_ansi_escapes("plain text"), "plain text");
        assert_eq!(strip_ansi_escapes("\x1b[1;31merror\x1b[0m: bad"), "error: bad");
        assert_eq!(strip_ansi_escapes("\x1b[2J\x1b[H"), "");
        assert_eq!(strip_ansi_escapes(""), "");
    }

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize("hello"), "Hello");
        assert_eq!(capitalize(""), "");
        assert_eq!(capitalize("HELLO"), "HELLO");
        assert_eq!(capitalize("a"), "A");
    }

    #[test]
    fn test_kebab_to_title() {
        assert_eq!(kebab_to_title("hello-world"), "Hello World");
        assert_eq!(kebab_to_title("my-awesome-project"), "My Awesome Project");
        assert_eq!(kebab_to_title("single"), "Single");
    }

    #[test]
    fn test_snake_to_title() {
        assert_eq!(snake_to_title("hello_world"), "Hello World");
        assert_eq!(snake_to_title("my_awesome_function"), "My Awesome Function");
    }

    #[test]
    fn test_camel_to_title() {
        assert_eq!(camel_to_title("helloWorld"), "Hello World");
        assert_eq!(camel_to_title("myAwesomeFunction"), "My Awesome Function");
        assert_eq!(camel_to_title("simple"), "Simple");
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("file/name?.txt"), "file_name_.txt");
        assert_eq!(sanitize_filename("valid_name.rs"), "valid_name.rs");
        assert_eq!(sanitize_filename("bad:file*name.txt"), "bad_file_name.txt");
        assert_eq!(sanitize_filename("C:\\path\\file.txt"), "C__path_file.txt");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("Hello World", 8, "..."), "Hello...");
        assert_eq!(truncate("Short", 10, "..."), "Short");
        assert_eq!(truncate("Exact", 5, "..."), "Exact");
    }

    #[test]
    fn test_truncate_middle() {
        assert_eq!(truncate_middle("HelloWorld", 8), "He...ld");
        assert_eq!(truncate_middle("Short", 10), "Short");
        assert_eq!(truncate_middle("VeryLongFileName", 12), "Very...Name");
        assert_eq!(truncate_middle("LongText", 8), "LongText"); // Same length, no truncation
        assert_eq!(truncate_middle("LongText", 7), "Lo...xt");
    }

    #[test]
    fn test_normalize_whitespace() {
        assert_eq!(normalize_whitespace("hello    world"), "hello world");
        assert_eq!(normalize_whitespace("  trim  me  "), "trim me");
        assert_eq!(normalize_whitespace("normal text"), "normal text");
    }

    #[test]
    fn test_word_count() {
        assert_eq!(word_count("hello world"), 2);
        assert_eq!(word_count("one"), 1);
        assert_eq!(word_count(""), 0);
        assert_eq!(word_count("  multiple   spaces  "), 2);
    }

    #[test]
    fn test_pluralize() {
        assert_eq!(pluralize(1, "file", None), "1 file");
        assert_eq!(pluralize(2, "file", None), "2 files");
        assert_eq!(pluralize(0, "file", None), "0 files");
        assert_eq!(pluralize(2, "child", Some("children")), "2 children");
        assert_eq!(pluralize(1, "child", Some("children")), "1 child");
    }
}
