//! Fuzzy Match — chain of 6 strategies for whitespace-tolerant string matching.
//!
//! The LLM often produces slightly different whitespace/indentation when
//! referencing code. This module tries each strategy in order; the first
//! one that finds **exactly 1 match** wins.
//!
//! Strategies:
//! 1. Exact match
//! 2. Line-trimmed (trim both sides per line)
//! 3. Indentation-flexible (strip leading whitespace)
//! 4. Block-anchor (Levenshtein on middle lines, first/last anchors)
//! 5. Escape-normalized (unescape `\n`, `\t`, `\"`, unicode quotes)
//! 6. Whitespace-normalized (collapse all whitespace)

/// Result of a successful fuzzy match.
#[derive(Debug, Clone, PartialEq)]
pub struct FuzzyMatch {
    /// Byte offset where the match starts in the original content.
    pub start: usize,
    /// Byte offset where the match ends in the original content.
    pub end: usize,
    /// Name of the strategy that matched.
    pub strategy: &'static str,
}

/// Try to find `search` in `content` using a chain of increasingly
/// flexible matchers. Returns `None` if no strategy finds exactly one match.
pub fn fuzzy_find(content: &str, search: &str) -> Option<FuzzyMatch> {
    // Strategy 1: Exact match
    if let Some(m) = exact_match(content, search) {
        return Some(m);
    }
    // Strategy 2: Line-trimmed match
    if let Some(m) = line_trimmed_match(content, search) {
        return Some(m);
    }
    // Strategy 3: Indentation-flexible match
    if let Some(m) = indentation_flexible_match(content, search) {
        return Some(m);
    }
    // Strategy 4: Block-anchor match (Levenshtein)
    if let Some(m) = block_anchor_match(content, search) {
        return Some(m);
    }
    // Strategy 5: Escape-normalized match
    if let Some(m) = escape_normalized_match(content, search) {
        return Some(m);
    }
    // Strategy 6: Whitespace-normalized match
    whitespace_normalized_match(content, search)
}

/// Find **all** fuzzy matches of `search` in `content`, cascading through
/// strategies. Returns all matches at the first strategy level that produces
/// any results. Used for `replace_all` support.
pub fn fuzzy_find_all(content: &str, search: &str) -> Vec<FuzzyMatch> {
    let exact = find_all_exact(content, search);
    if !exact.is_empty() {
        return exact;
    }
    let trimmed = find_all_line_trimmed(content, search);
    if !trimmed.is_empty() {
        return trimmed;
    }
    let indent = find_all_indentation_flexible(content, search);
    if !indent.is_empty() {
        return indent;
    }
    // block_anchor is inherently single-match (anchored block), skip for find_all
    let escape = find_all_escape_normalized(content, search);
    if !escape.is_empty() {
        return escape;
    }
    let ws = find_all_whitespace_normalized(content, search);
    if !ws.is_empty() {
        return ws;
    }
    vec![]
}

// ============================================================================
// STRATEGY 1: Exact match
// ============================================================================

fn exact_match(content: &str, search: &str) -> Option<FuzzyMatch> {
    let first = content.find(search)?;
    // Verify uniqueness: rfind must return the same position
    let last = content.rfind(search)?;
    if first != last {
        return None; // Multiple matches
    }
    Some(FuzzyMatch {
        start: first,
        end: first + search.len(),
        strategy: "exact",
    })
}

// ============================================================================
// STRATEGY 2: Line-trimmed match
// ============================================================================

/// Trim each line on both sides, then try to find the search block.
/// Maps back to original positions.
fn line_trimmed_match(content: &str, search: &str) -> Option<FuzzyMatch> {
    let trimmed_search: Vec<&str> = search.lines().map(|l| l.trim()).collect();
    if trimmed_search.is_empty() {
        return None;
    }

    let content_lines: Vec<(usize, &str)> = line_offsets(content);
    let search_line_count = trimmed_search.len();

    let mut matches = Vec::new();

    for window_start in 0..content_lines.len().saturating_sub(search_line_count - 1) {
        let window = &content_lines[window_start..window_start + search_line_count];
        let all_match = window.iter().zip(&trimmed_search).all(|((_offset, line), search_line)| {
            line.trim() == *search_line
        });

        if all_match {
            let start = window[0].0;
            let last = &window[search_line_count - 1];
            let end = last.0 + last.1.len();
            let end = adjust_end_for_newline(content, end);
            matches.push((start, end));
        }
    }

    if matches.len() == 1 {
        Some(FuzzyMatch {
            start: matches[0].0,
            end: matches[0].1,
            strategy: "line_trimmed",
        })
    } else {
        None
    }
}

// ============================================================================
// STRATEGY 3: Indentation-flexible match
// ============================================================================

/// Strip leading whitespace from each line (keep 1 space as separator),
/// then try to find the normalized search block in the normalized content.
fn indentation_flexible_match(content: &str, search: &str) -> Option<FuzzyMatch> {
    let content_lines: Vec<(usize, &str)> = line_offsets(content);
    let search_lines: Vec<&str> = search.lines().collect();

    if search_lines.is_empty() || content_lines.is_empty() {
        return None;
    }

    let normalized_search: Vec<String> = search_lines
        .iter()
        .map(|l| l.trim_start().to_string())
        .collect();

    let search_line_count = normalized_search.len();
    let mut matches = Vec::new();

    for window_start in 0..content_lines.len().saturating_sub(search_line_count - 1) {
        let window = &content_lines[window_start..window_start + search_line_count];
        let all_match = window.iter().zip(&normalized_search).all(|((_offset, line), norm)| {
            line.trim_start() == norm.as_str()
        });

        if all_match {
            let start = window[0].0;
            let last = &window[search_line_count - 1];
            let end = last.0 + last.1.len();
            let end = adjust_end_for_newline(content, end);
            matches.push((start, end));
        }
    }

    if matches.len() == 1 {
        Some(FuzzyMatch {
            start: matches[0].0,
            end: matches[0].1,
            strategy: "indentation_flexible",
        })
    } else {
        None
    }
}

// ============================================================================
// STRATEGY 4: Block-anchor match (Levenshtein)
// ============================================================================

/// Match first and last lines (trimmed) as anchors, then score the middle
/// block using Levenshtein similarity. Handles LLM omitting/modifying interior
/// lines while keeping first/last correct. Requires ≥3 lines in search.
fn block_anchor_match(content: &str, search: &str) -> Option<FuzzyMatch> {
    let search_lines: Vec<&str> = search.lines().collect();
    if search_lines.len() < 3 {
        return None;
    }

    let first_search = search_lines.first()?.trim();
    let last_search = search_lines.last()?.trim();

    if first_search.is_empty() || last_search.is_empty() {
        return None;
    }

    let content_lines = line_offsets(content);

    // Find candidate pairs where first and last lines match (trimmed)
    let mut candidates: Vec<(usize, usize)> = Vec::new();
    for i in 0..content_lines.len() {
        if content_lines[i].1.trim() != first_search {
            continue;
        }
        for j in (i + 2)..content_lines.len() {
            if content_lines[j].1.trim() == last_search {
                candidates.push((i, j));
                break; // take nearest matching end for this start
            }
        }
    }

    if candidates.is_empty() {
        return None;
    }

    let threshold = if candidates.len() == 1 { 0.0 } else { 0.3 };

    // Score each candidate by middle-line Levenshtein similarity
    let search_middle: String = search_lines[1..search_lines.len() - 1].join("\n");

    let mut best: Option<(f64, usize, usize)> = None;
    for &(i, j) in &candidates {
        let content_middle: String = content_lines[i + 1..j]
            .iter()
            .map(|(_, line)| *line)
            .collect::<Vec<_>>()
            .join("\n");

        let sim = levenshtein_similarity(&search_middle, &content_middle);
        if sim >= threshold
            && best.is_none_or(|(best_sim, _, _)| sim > best_sim) {
                best = Some((sim, i, j));
            }
    }

    let (_, start_idx, end_idx) = best?;

    let start = content_lines[start_idx].0;
    let last_line = &content_lines[end_idx];
    let end = last_line.0 + last_line.1.len();
    let end = adjust_end_for_newline(content, end);

    Some(FuzzyMatch {
        start,
        end,
        strategy: "block_anchor",
    })
}

// ============================================================================
// STRATEGY 5: Escape-normalized match
// ============================================================================

/// The LLM sometimes produces escaped characters (`\n`, `\t`, `\"`) or
/// unicode curly quotes instead of ASCII quotes. Unescape and try exact match.
fn escape_normalized_match(content: &str, search: &str) -> Option<FuzzyMatch> {
    let unescaped = unescape(search);
    if unescaped == search {
        return None; // no escapes or unicode quotes found, skip
    }
    exact_match(content, &unescaped).map(|m| FuzzyMatch {
        strategy: "escape_normalized",
        ..m
    })
}

// ============================================================================
// STRATEGY 6: Whitespace-normalized match
// ============================================================================

/// Collapse all runs of whitespace (including newlines) to a single space,
/// then find the search in content. Map back to original byte offsets.
fn whitespace_normalized_match(content: &str, search: &str) -> Option<FuzzyMatch> {
    let (norm_content, content_map) = normalize_whitespace_with_map(content);
    let norm_search = normalize_whitespace(search);

    if norm_search.is_empty() {
        return None;
    }

    let first = norm_content.find(&norm_search)?;
    // Verify uniqueness
    let last = norm_content.rfind(&norm_search)?;
    if first != last {
        return None;
    }

    let end_norm = first + norm_search.len();

    // Map normalized positions back to original byte offsets
    let start_orig = content_map.get(first).copied()?;
    // end_norm might be past the last mapped char — use the end of the last mapped char
    let end_orig = if end_norm >= content_map.len() {
        content.len()
    } else {
        content_map.get(end_norm).copied()?
    };

    Some(FuzzyMatch {
        start: start_orig,
        end: end_orig,
        strategy: "whitespace_normalized",
    })
}

// ============================================================================
// HELPERS
// ============================================================================

/// Returns Vec of (byte_offset, line_content) for each line.
fn line_offsets(content: &str) -> Vec<(usize, &str)> {
    let mut result = Vec::new();
    let mut offset = 0;
    for line in content.lines() {
        result.push((offset, line));
        offset += line.len();
        // Skip the newline character(s)
        if content.as_bytes().get(offset) == Some(&b'\r') {
            offset += 1;
        }
        if content.as_bytes().get(offset) == Some(&b'\n') {
            offset += 1;
        }
    }
    result
}

/// Collapse whitespace and return a mapping from normalized char index to original byte offset.
fn normalize_whitespace_with_map(s: &str) -> (String, Vec<usize>) {
    let mut result = String::new();
    let mut map = Vec::new(); // map[normalized_char_idx] = original_byte_offset
    let mut in_ws = false;

    for (byte_offset, ch) in s.char_indices() {
        if ch.is_whitespace() {
            if !in_ws {
                result.push(' ');
                map.push(byte_offset);
                in_ws = true;
            }
        } else {
            result.push(ch);
            map.push(byte_offset);
            in_ws = false;
        }
    }

    (result, map)
}

/// Collapse whitespace runs to a single space (no mapping).
fn normalize_whitespace(s: &str) -> String {
    let mut result = String::new();
    let mut in_ws = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !in_ws {
                result.push(' ');
                in_ws = true;
            }
        } else {
            result.push(ch);
            in_ws = false;
        }
    }
    result
}

/// Advance `end` past a trailing newline (LF or CRLF) if present.
fn adjust_end_for_newline(content: &str, end: usize) -> usize {
    if content.as_bytes().get(end) == Some(&b'\n') {
        end + 1
    } else if content.as_bytes().get(end) == Some(&b'\r') {
        if content.as_bytes().get(end + 1) == Some(&b'\n') {
            end + 2
        } else {
            end + 1
        }
    } else {
        end
    }
}

/// Levenshtein edit distance between two strings (char-level).
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0; b_len + 1];

    for i in 0..a_len {
        curr[0] = i + 1;
        for j in 0..b_len {
            let cost = if a_chars[i] == b_chars[j] { 0 } else { 1 };
            curr[j + 1] = (prev[j] + cost)
                .min(prev[j + 1] + 1)
                .min(curr[j] + 1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b_len]
}

/// Similarity score (0.0 = completely different, 1.0 = identical).
fn levenshtein_similarity(a: &str, b: &str) -> f64 {
    let max_len = a.chars().count().max(b.chars().count());
    if max_len == 0 {
        return 1.0;
    }
    let dist = levenshtein(a, b);
    1.0 - (dist as f64 / max_len as f64)
}

/// Unescape common LLM escape sequences and normalize unicode quotes.
fn unescape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.peek() {
                Some('n') => {
                    chars.next();
                    result.push('\n');
                }
                Some('t') => {
                    chars.next();
                    result.push('\t');
                }
                Some('"') => {
                    chars.next();
                    result.push('"');
                }
                Some('\\') => {
                    chars.next();
                    result.push('\\');
                }
                _ => result.push(ch),
            }
        } else if ch == '\u{201C}' || ch == '\u{201D}' {
            // Unicode curly quotes → ASCII
            result.push('"');
        } else {
            result.push(ch);
        }
    }
    result
}

// ============================================================================
// FIND-ALL HELPERS (for fuzzy_find_all / replace_all)
// ============================================================================

fn find_all_exact(content: &str, search: &str) -> Vec<FuzzyMatch> {
    if search.is_empty() {
        return vec![];
    }
    let mut matches = Vec::new();
    let mut start = 0;
    while let Some(pos) = content[start..].find(search) {
        let abs_pos = start + pos;
        matches.push(FuzzyMatch {
            start: abs_pos,
            end: abs_pos + search.len(),
            strategy: "exact",
        });
        start = abs_pos + search.len();
    }
    matches
}

fn find_all_line_trimmed(content: &str, search: &str) -> Vec<FuzzyMatch> {
    let trimmed_search: Vec<&str> = search.lines().map(|l| l.trim()).collect();
    if trimmed_search.is_empty() {
        return vec![];
    }

    let content_lines = line_offsets(content);
    let search_line_count = trimmed_search.len();
    let mut matches = Vec::new();

    let mut window_start = 0;
    while window_start + search_line_count <= content_lines.len() {
        let window = &content_lines[window_start..window_start + search_line_count];
        let all_match = window
            .iter()
            .zip(&trimmed_search)
            .all(|((_, line), search_line)| line.trim() == *search_line);

        if all_match {
            let start = window[0].0;
            let last = &window[search_line_count - 1];
            let end = last.0 + last.1.len();
            let end = adjust_end_for_newline(content, end);
            matches.push(FuzzyMatch {
                start,
                end,
                strategy: "line_trimmed",
            });
            window_start += search_line_count; // skip past this match
        } else {
            window_start += 1;
        }
    }

    matches
}

fn find_all_indentation_flexible(content: &str, search: &str) -> Vec<FuzzyMatch> {
    let content_lines = line_offsets(content);
    let search_lines: Vec<&str> = search.lines().collect();
    if search_lines.is_empty() || content_lines.is_empty() {
        return vec![];
    }

    let normalized_search: Vec<String> = search_lines
        .iter()
        .map(|l| l.trim_start().to_string())
        .collect();
    let search_line_count = normalized_search.len();
    let mut matches = Vec::new();

    let mut window_start = 0;
    while window_start + search_line_count <= content_lines.len() {
        let window = &content_lines[window_start..window_start + search_line_count];
        let all_match = window
            .iter()
            .zip(&normalized_search)
            .all(|((_, line), norm)| line.trim_start() == norm.as_str());

        if all_match {
            let start = window[0].0;
            let last = &window[search_line_count - 1];
            let end = last.0 + last.1.len();
            let end = adjust_end_for_newline(content, end);
            matches.push(FuzzyMatch {
                start,
                end,
                strategy: "indentation_flexible",
            });
            window_start += search_line_count;
        } else {
            window_start += 1;
        }
    }

    matches
}

fn find_all_escape_normalized(content: &str, search: &str) -> Vec<FuzzyMatch> {
    let unescaped = unescape(search);
    if unescaped == search {
        return vec![];
    }
    find_all_exact(content, &unescaped)
        .into_iter()
        .map(|m| FuzzyMatch {
            strategy: "escape_normalized",
            ..m
        })
        .collect()
}

fn find_all_whitespace_normalized(content: &str, search: &str) -> Vec<FuzzyMatch> {
    let (norm_content, content_map) = normalize_whitespace_with_map(content);
    let norm_search = normalize_whitespace(search);
    if norm_search.is_empty() {
        return vec![];
    }

    let mut matches = Vec::new();
    let mut start = 0;
    while let Some(pos) = norm_content[start..].find(&norm_search) {
        let abs_pos = start + pos;
        let end_norm = abs_pos + norm_search.len();

        if let Some(&start_orig) = content_map.get(abs_pos) {
            let end_orig = if end_norm >= content_map.len() {
                content.len()
            } else if let Some(&e) = content_map.get(end_norm) {
                e
            } else {
                content.len()
            };

            matches.push(FuzzyMatch {
                start: start_orig,
                end: end_orig,
                strategy: "whitespace_normalized",
            });
        }

        start = abs_pos + norm_search.len();
    }

    matches
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- Strategy 1: Exact ---

    #[test]
    fn test_exact_match_found() {
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let search = "println!(\"hello\");";
        let m = fuzzy_find(content, search).unwrap();
        assert_eq!(m.strategy, "exact");
        assert_eq!(&content[m.start..m.end], search);
    }

    #[test]
    fn test_exact_match_not_found() {
        let content = "fn main() {}";
        let search = "fn other() {}";
        assert!(fuzzy_find(content, search).is_none());
    }

    #[test]
    fn test_exact_match_multiple_rejects() {
        let content = "let x = 1;\nlet x = 1;\n";
        let search = "let x = 1;";
        // Exact finds 2 matches, falls through to line_trimmed which also finds 2, etc.
        // All strategies should fail on duplicates
        assert!(exact_match(content, search).is_none());
    }

    // --- Strategy 2: Line-trimmed ---

    #[test]
    fn test_line_trimmed_match() {
        let content = "fn main() {\n    let x = 1;\n    let y = 2;\n}\n";
        // LLM output might lose indentation
        let search = "let x = 1;\nlet y = 2;";
        let m = fuzzy_find(content, search).unwrap();
        assert_eq!(m.strategy, "line_trimmed");
        // The matched region should contain the original indented lines
        let matched = &content[m.start..m.end];
        assert!(matched.contains("let x = 1;"));
        assert!(matched.contains("let y = 2;"));
    }

    #[test]
    fn test_line_trimmed_extra_spaces() {
        let content = "  hello world  \n  foo bar  \n";
        let search = "hello world\nfoo bar";
        let m = fuzzy_find(content, search).unwrap();
        assert!(m.strategy == "line_trimmed" || m.strategy == "exact");
    }

    // --- Strategy 3: Indentation-flexible ---

    #[test]
    fn test_indentation_flexible_match() {
        // Content has trailing spaces that differ from search — line_trimmed won't match
        // because trim() strips trailing too, but the *content* has trailing chars
        // that only differ in leading whitespace.
        let content = "    fn foo() {  \n        let x = 1;  \n    }\n";
        // Search has different indentation AND no trailing spaces
        let search = "  fn foo() {\n      let x = 1;\n  }";
        let m = fuzzy_find(content, search).unwrap();
        // line_trimmed trims both sides so trailing spaces are stripped — it matches first
        // Both strategies handle this; accept either
        assert!(
            m.strategy == "indentation_flexible" || m.strategy == "line_trimmed",
            "expected indentation_flexible or line_trimmed, got: {}", m.strategy
        );
        let matched = &content[m.start..m.end];
        assert!(matched.contains("fn foo()"));
        assert!(matched.contains("let x = 1;"));
    }

    #[test]
    fn test_indentation_flexible_multiline() {
        // Multi-line block with tab indent in content, space indent in search
        let content = "fn main() {\n\tlet a = 1;\n\tlet b = 2;\n}\n";
        let search = "    let a = 1;\n    let b = 2;";
        let m = fuzzy_find(content, search).unwrap();
        // Both strategies handle leading whitespace differences
        assert!(
            m.strategy == "indentation_flexible" || m.strategy == "line_trimmed",
            "got: {}", m.strategy
        );
        let matched = &content[m.start..m.end];
        assert!(matched.contains("let a = 1;"));
        assert!(matched.contains("let b = 2;"));
    }

    #[test]
    fn test_indentation_tabs_vs_spaces() {
        let content = "\tfn bar() {\n\t\tlet y = 2;\n\t}\n";
        let search = "fn bar() {\n    let y = 2;\n}";
        let m = fuzzy_find(content, search).unwrap();
        assert!(m.strategy == "indentation_flexible" || m.strategy == "line_trimmed");
    }

    // --- Strategy 4: Block-anchor ---

    #[test]
    fn test_block_anchor_llm_omits_middle_lines() {
        // Content has 5 lines in the function body
        let content = r#"fn process() {
    let a = 1;
    let b = 2;
    let c = 3;
    let d = 4;
}
"#;
        // LLM only kept first/last lines, changed middle
        let search = "fn process() {\n    let x = modified;\n}";
        let m = fuzzy_find(content, search).unwrap();
        assert_eq!(m.strategy, "block_anchor");
        let matched = &content[m.start..m.end];
        assert!(matched.starts_with("fn process()"));
        assert!(matched.contains("let d = 4;"));
        assert!(matched.ends_with("}\n"));
    }

    #[test]
    fn test_block_anchor_multiple_candidates_picks_best() {
        // Two blocks with the same first/last lines — neither is exact match
        // block_anchor should pick the one with best middle similarity
        let content = "fn run() {\n    setup();\n    init();\n}\nfn run() {\n    execute();\n    cleanup();\n}\n";
        // Search middle is closer to second block
        let search = "fn run() {\n    execute_all();\n    cleanup_all();\n}";
        let m = fuzzy_find(content, search).unwrap();
        assert_eq!(m.strategy, "block_anchor");
        let matched = &content[m.start..m.end];
        assert!(matched.contains("execute()"));
        assert!(matched.contains("cleanup()"));
    }

    #[test]
    fn test_block_anchor_rejects_less_than_3_lines() {
        let content = "fn foo() {\n    bar();\n}\n";
        let search = "fn foo() {\n}";
        // Only 2 lines — block_anchor should not fire, but other strategies may match
        let m = block_anchor_match(content, search);
        assert!(m.is_none());
    }

    // --- Strategy 5: Escape-normalized ---

    #[test]
    fn test_escape_normalized_newline() {
        let content = "line one\nline two\n";
        // LLM produced escaped \n instead of actual newline
        let search = "line one\\nline two";
        let m = fuzzy_find(content, search).unwrap();
        assert_eq!(m.strategy, "escape_normalized");
        assert_eq!(&content[m.start..m.end], "line one\nline two");
    }

    #[test]
    fn test_escape_normalized_tab_and_quotes() {
        let content = "let x = \t\"hello\";\n";
        let search = "let x = \\t\\\"hello\\\";";
        let m = fuzzy_find(content, search).unwrap();
        assert_eq!(m.strategy, "escape_normalized");
    }

    #[test]
    fn test_escape_normalized_unicode_quotes() {
        let content = "let msg = \"world\";\n";
        let search = "let msg = \u{201C}world\u{201D};";
        let m = fuzzy_find(content, search).unwrap();
        assert_eq!(m.strategy, "escape_normalized");
    }

    #[test]
    fn test_escape_normalized_noop_if_no_escapes() {
        let content = "let x = 1;\n";
        let search = "let x = 1;";
        // No escapes in search — should use exact, not escape_normalized
        let m = fuzzy_find(content, search).unwrap();
        assert_eq!(m.strategy, "exact");
    }

    // --- Strategy 6: Whitespace-normalized ---

    #[test]
    fn test_whitespace_normalized_match() {
        let content = "fn   main()  {\n    println!(  \"hello\"  );\n}\n";
        let search = "fn main() { println!( \"hello\" ); }";
        let m = fuzzy_find(content, search).unwrap();
        assert_eq!(m.strategy, "whitespace_normalized");
    }

    #[test]
    fn test_whitespace_normalized_newline_collapse() {
        let content = "let a = 1;\n\n\nlet b = 2;\n";
        let search = "let a = 1; let b = 2;";
        let m = fuzzy_find(content, search).unwrap();
        assert_eq!(m.strategy, "whitespace_normalized");
    }

    // --- Integration ---

    #[test]
    fn test_prefers_exact_over_fuzzy() {
        let content = "fn hello() {}\n";
        let search = "fn hello() {}";
        let m = fuzzy_find(content, search).unwrap();
        assert_eq!(m.strategy, "exact");
    }

    #[test]
    fn test_empty_search_returns_none() {
        let content = "some content";
        assert!(fuzzy_find(content, "").is_none());
    }

    #[test]
    fn test_empty_content_returns_none() {
        assert!(fuzzy_find("", "search").is_none());
    }

    #[test]
    fn test_real_world_indentation_mismatch() {
        let content = r#"impl Config {
    pub fn new() -> Self {
        Self {
            debug: false,
            verbose: false,
        }
    }
}
"#;
        // LLM might produce 2-space indent instead of 4-space
        let search = r#"pub fn new() -> Self {
  Self {
    debug: false,
    verbose: false,
  }
}"#;
        let m = fuzzy_find(content, search).unwrap();
        assert!(m.strategy == "indentation_flexible" || m.strategy == "line_trimmed");
    }

    // --- fuzzy_find_all ---

    #[test]
    fn test_find_all_exact_multiple() {
        let content = "let x = TODO;\nlet y = TODO;\nlet z = TODO;\n";
        let matches = fuzzy_find_all(content, "TODO");
        assert_eq!(matches.len(), 3);
        assert!(matches.iter().all(|m| m.strategy == "exact"));
        assert_eq!(&content[matches[0].start..matches[0].end], "TODO");
        assert_eq!(&content[matches[1].start..matches[1].end], "TODO");
        assert_eq!(&content[matches[2].start..matches[2].end], "TODO");
    }

    #[test]
    fn test_find_all_line_trimmed_multiple() {
        let content = "    foo();\n    bar();\n    foo();\n    baz();\n";
        // Search without indentation — line_trimmed should find both "foo();"
        let matches = fuzzy_find_all(content, "foo();");
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_find_all_empty_on_no_match() {
        let content = "fn main() {}\n";
        let matches = fuzzy_find_all(content, "nonexistent_function");
        assert!(matches.is_empty());
    }

    #[test]
    fn test_find_all_escape_normalized() {
        let content = "print(\"a\")\nprint(\"b\")\n";
        // Search with escaped quotes
        let search = "print(\\\"a\\\")";
        let matches = fuzzy_find_all(content, search);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].strategy, "escape_normalized");
    }

    // --- Helpers ---

    #[test]
    fn test_levenshtein_identical() {
        assert_eq!(levenshtein("hello", "hello"), 0);
    }

    #[test]
    fn test_levenshtein_one_edit() {
        assert_eq!(levenshtein("hello", "hallo"), 1);
    }

    #[test]
    fn test_levenshtein_empty() {
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", ""), 3);
    }

    #[test]
    fn test_similarity_identical() {
        let s = levenshtein_similarity("hello", "hello");
        assert!((s - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_similarity_completely_different() {
        let s = levenshtein_similarity("abc", "xyz");
        assert!(s < 0.01);
    }

    #[test]
    fn test_unescape_basic() {
        assert_eq!(unescape(r#"hello\nworld"#), "hello\nworld");
        assert_eq!(unescape(r#"\t\""#), "\t\"");
        assert_eq!(unescape(r#"\\"#), "\\");
    }

    #[test]
    fn test_unescape_unicode_quotes() {
        assert_eq!(unescape("\u{201C}hello\u{201D}"), "\"hello\"");
    }
}
