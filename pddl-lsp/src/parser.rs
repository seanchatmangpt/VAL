use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

impl Span {
    pub fn contains(&self, offset: usize) -> bool {
        self.start <= offset && offset <= self.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum TokenKind {
    LeftParen,
    RightParen,
    Atom,
    Comment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum SExprKind {
    Atom(String),
    List(Vec<SExpr>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SExpr {
    pub kind: SExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParseIssue {
    pub code: &'static str,
    pub message: String,
    pub span: Span,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ParseResult {
    pub tokens: Vec<Token>,
    pub roots: Vec<SExpr>,
    pub issues: Vec<ParseIssue>,
}

/// Maximum list nesting depth the parser builds into the tree. Every consumer
/// of [`SExpr`] (semantic walkers, clone, drop, serialization) recurses over
/// the tree, so the tree depth is bounded here, once, instead of trusting input.
/// A list opened deeper than this is refused with `PDDL_NESTING_TOO_DEEP`, its
/// balanced extent is skipped iteratively, and an empty list stands in for it.
/// Real PDDL/HDDL/FOND documents nest well under 64 levels.
pub const MAX_NESTING_DEPTH: usize = 256;

pub fn parse(text: &str) -> ParseResult {
    let tokens = tokenize(text);
    let mut cursor = 0usize;
    let mut roots = Vec::new();
    let mut issues = Vec::new();

    while cursor < tokens.len() {
        if matches!(tokens[cursor].kind, TokenKind::Comment) {
            cursor += 1;
            continue;
        }
        match parse_expr(&tokens, &mut cursor, &mut issues, 0) {
            Some(expr) => roots.push(expr),
            None => cursor += 1,
        }
    }

    ParseResult { tokens, roots, issues }
}

/// Skips the balanced list whose `(` is at `*cursor`, without recursion.
/// Returns the span of the skipped extent and whether it was closed.
fn skip_list(tokens: &[Token], cursor: &mut usize) -> (Span, bool) {
    let start = tokens[*cursor].span;
    let mut open = 0usize;
    while *cursor < tokens.len() {
        let token = &tokens[*cursor];
        *cursor += 1;
        match token.kind {
            TokenKind::LeftParen => open += 1,
            TokenKind::RightParen => {
                open -= 1;
                if open == 0 {
                    let end = token.span;
                    return (
                        Span {
                            start: start.start,
                            end: end.end,
                            start_line: start.start_line,
                            start_character: start.start_character,
                            end_line: end.end_line,
                            end_character: end.end_character,
                        },
                        true,
                    );
                }
            }
            TokenKind::Atom | TokenKind::Comment => {}
        }
    }
    (start, false)
}

fn parse_expr(tokens: &[Token], cursor: &mut usize, issues: &mut Vec<ParseIssue>, depth: usize) -> Option<SExpr> {
    let token = tokens.get(*cursor)?.clone();
    match token.kind {
        TokenKind::Atom => {
            *cursor += 1;
            Some(SExpr { kind: SExprKind::Atom(token.text), span: token.span })
        }
        TokenKind::Comment => {
            *cursor += 1;
            None
        }
        TokenKind::RightParen => {
            issues.push(ParseIssue {
                code: "PDDL_UNMATCHED_RIGHT_PAREN",
                message: "unmatched ')'".to_string(),
                span: token.span,
            });
            None
        }
        TokenKind::LeftParen if depth >= MAX_NESTING_DEPTH => {
            let (span, closed) = skip_list(tokens, cursor);
            issues.push(ParseIssue {
                code: "PDDL_NESTING_TOO_DEEP",
                message: format!("list nesting exceeds {MAX_NESTING_DEPTH} levels; the nested form was not analyzed"),
                span,
            });
            if !closed {
                issues.push(ParseIssue {
                    code: "PDDL_UNCLOSED_LIST",
                    message: "list opened here is never closed".to_string(),
                    span,
                });
            }
            Some(SExpr { kind: SExprKind::List(Vec::new()), span })
        }
        TokenKind::LeftParen => {
            *cursor += 1;
            let mut children = Vec::new();
            let start = token.span;
            while *cursor < tokens.len() {
                match tokens[*cursor].kind {
                    TokenKind::RightParen => {
                        let end = tokens[*cursor].span;
                        *cursor += 1;
                        return Some(SExpr {
                            kind: SExprKind::List(children),
                            span: Span {
                                start: start.start,
                                end: end.end,
                                start_line: start.start_line,
                                start_character: start.start_character,
                                end_line: end.end_line,
                                end_character: end.end_character,
                            },
                        });
                    }
                    TokenKind::Comment => *cursor += 1,
                    _ => {
                        if let Some(expr) = parse_expr(tokens, cursor, issues, depth + 1) {
                            children.push(expr);
                        }
                    }
                }
            }
            issues.push(ParseIssue {
                code: "PDDL_UNCLOSED_LIST",
                message: "list opened here is never closed".to_string(),
                span: start,
            });
            Some(SExpr { kind: SExprKind::List(children), span: start })
        }
    }
}

pub fn tokenize(text: &str) -> Vec<Token> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    let mut line = 0u32;
    let mut column = 0u32;

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\n' {
            i += 1;
            line += 1;
            column = 0;
            continue;
        }
        if b.is_ascii_whitespace() {
            i += 1;
            column += 1;
            continue;
        }
        let start = i;
        let start_line = line;
        let start_column = column;
        if b == b';' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
                column += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Comment,
                text: text[start..i].to_string(),
                span: Span {
                    start,
                    end: i,
                    start_line,
                    start_character: start_column,
                    end_line: line,
                    end_character: column,
                },
            });
            continue;
        }
        let kind = if b == b'(' {
            i += 1;
            column += 1;
            TokenKind::LeftParen
        } else if b == b')' {
            i += 1;
            column += 1;
            TokenKind::RightParen
        } else {
            while i < bytes.len()
                && !bytes[i].is_ascii_whitespace()
                && bytes[i] != b'('
                && bytes[i] != b')'
                && bytes[i] != b';'
            {
                i += 1;
                column += 1;
            }
            TokenKind::Atom
        };
        tokens.push(Token {
            kind,
            text: text[start..i].to_string(),
            span: Span {
                start,
                end: i,
                start_line,
                start_character: start_column,
                end_line: line,
                end_character: column,
            },
        });
    }
    tokens
}

pub fn atom(expr: &SExpr) -> Option<&str> {
    match &expr.kind {
        SExprKind::Atom(value) => Some(value),
        SExprKind::List(_) => None,
    }
}

pub fn list(expr: &SExpr) -> Option<&[SExpr]> {
    match &expr.kind {
        SExprKind::List(items) => Some(items),
        SExprKind::Atom(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_receipts_balanced_document() {
        let result = parse("(define (domain logistics) (:predicates (at ?x)))");
        assert!(result.issues.is_empty());
        assert_eq!(result.roots.len(), 1);
    }

    fn nested(depth: usize) -> String { format!("{}{}", "(".repeat(depth), ")".repeat(depth)) }

    fn tree_depth(expr: &SExpr) -> usize {
        // Iterative so the measurement itself cannot overflow.
        let mut max = 0;
        let mut stack = vec![(expr, 1usize)];
        while let Some((e, d)) = stack.pop() {
            max = max.max(d);
            if let SExprKind::List(items) = &e.kind { stack.extend(items.iter().map(|c| (c, d + 1))); }
        }
        max
    }

    #[test]
    fn nesting_at_the_bound_is_admitted() {
        let result = parse(&nested(MAX_NESTING_DEPTH));
        assert!(result.issues.is_empty(), "{:?}", result.issues);
        assert_eq!(tree_depth(&result.roots[0]), MAX_NESTING_DEPTH);
    }

    #[test]
    fn nesting_past_the_bound_is_refused_and_tree_depth_is_bounded() {
        for depth in [MAX_NESTING_DEPTH + 1, 5_000, 200_000] {
            let result = parse(&nested(depth));
            let deep: Vec<_> = result.issues.iter().filter(|i| i.code == "PDDL_NESTING_TOO_DEEP").collect();
            assert_eq!(deep.len(), 1, "depth {depth}: {:?}", result.issues);
            assert!(!result.issues.iter().any(|i| i.code == "PDDL_UNCLOSED_LIST"), "depth {depth} is balanced");
            assert_eq!(result.roots.len(), 1);
            assert_eq!(tree_depth(&result.roots[0]), MAX_NESTING_DEPTH + 1);
        }
    }

    #[test]
    fn unbalanced_deep_input_is_refused_as_too_deep_and_unclosed() {
        let result = parse(&"(".repeat(200_000));
        assert!(result.issues.iter().any(|i| i.code == "PDDL_NESTING_TOO_DEEP"));
        assert!(result.issues.iter().any(|i| i.code == "PDDL_UNCLOSED_LIST"));
    }

    #[test]
    fn parsing_resumes_after_a_skipped_deep_form() {
        let text = format!("(define (domain d) {} (:predicates (at ?x)))", nested(MAX_NESTING_DEPTH + 10));
        let result = parse(&text);
        assert_eq!(result.issues.iter().filter(|i| i.code == "PDDL_NESTING_TOO_DEEP").count(), 1);
        assert_eq!(result.roots.len(), 1);
        let SExprKind::List(items) = &result.roots[0].kind else { panic!("root is a list") };
        assert_eq!(items.len(), 4, "define, (domain d), skipped form, (:predicates ...)");
    }

    #[test]
    fn parser_refuses_unclosed_list() {
        let result = parse("(define (domain logistics)");
        assert!(result.issues.iter().any(|issue| issue.code == "PDDL_UNCLOSED_LIST"));
    }
}
