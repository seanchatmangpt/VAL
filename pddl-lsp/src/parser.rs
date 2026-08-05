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
        match parse_expr(&tokens, &mut cursor, &mut issues) {
            Some(expr) => roots.push(expr),
            None => cursor += 1,
        }
    }

    ParseResult { tokens, roots, issues }
}

fn parse_expr(tokens: &[Token], cursor: &mut usize, issues: &mut Vec<ParseIssue>) -> Option<SExpr> {
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
                        if let Some(expr) = parse_expr(tokens, cursor, issues) {
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

    #[test]
    fn parser_refuses_unclosed_list() {
        let result = parse("(define (domain logistics)");
        assert!(result.issues.iter().any(|issue| issue.code == "PDDL_UNCLOSED_LIST"));
    }
}
