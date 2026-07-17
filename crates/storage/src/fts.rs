//! Turn untrusted user search input into a safe FTS5 MATCH expression.
//!
//! FTS5 has its own query syntax (quotes, `*`, AND/OR/NOT, column filters). Passing
//! raw user text through would let a chat message inject operators or raise syntax
//! errors, so each whitespace-separated term is wrapped as a quoted string. Quoted
//! terms are ANDed implicitly — every term must appear. A trailing `*` on a bare
//! term is preserved as a prefix match.

/// Returns `None` when the input has no searchable term (caller should reject).
pub fn to_match_expr(input: &str) -> Option<String> {
    let terms: Vec<String> = input.split_whitespace().filter_map(sanitize_term).collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" "))
    }
}

fn sanitize_term(term: &str) -> Option<String> {
    let prefix = term.ends_with('*');
    let core = term.trim_end_matches('*');
    // Strip characters that are structural in FTS5 so only literal content remains.
    let cleaned: String = core.chars().filter(|c| !is_fts_syntax(*c)).collect();
    if cleaned.is_empty() {
        return None;
    }
    let escaped = cleaned.replace('"', "\"\"");
    Some(if prefix {
        format!("\"{escaped}\"*")
    } else {
        format!("\"{escaped}\"")
    })
}

fn is_fts_syntax(c: char) -> bool {
    matches!(c, '"' | '\'' | '(' | ')' | '*' | ':' | '^' | '-' | '+')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_terms_are_quoted_and_anded() {
        assert_eq!(
            to_match_expr("ошибка init").as_deref(),
            Some("\"ошибка\" \"init\"")
        );
    }

    #[test]
    fn prefix_star_is_preserved() {
        assert_eq!(to_match_expr("багфи*").as_deref(), Some("\"багфи\"*"));
    }

    #[test]
    fn fts_operators_are_neutralized() {
        // NEAR/AND/OR become literal quoted terms, not operators.
        let expr = to_match_expr("foo OR bar").unwrap();
        assert_eq!(expr, "\"foo\" \"OR\" \"bar\"");
    }

    #[test]
    fn injection_chars_are_stripped() {
        let expr = to_match_expr("a\"b(c)").unwrap();
        assert_eq!(expr, "\"abc\"");
    }

    #[test]
    fn empty_or_syntax_only_input_is_none() {
        assert_eq!(to_match_expr("   "), None);
        assert_eq!(to_match_expr("\"()*"), None);
    }
}
