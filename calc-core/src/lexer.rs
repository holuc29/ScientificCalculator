//! Turns input text into tokens.

use crate::error::CalcError;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Number(String),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Percent,
    Bang,
    Square,
    Cube,
    LParen,
    RParen,
    Comma,
    Assign,
    Eof,
}

impl Tok {
    pub fn describe(&self) -> String {
        match self {
            Tok::Number(s) | Tok::Ident(s) => s.clone(),
            Tok::Plus => "+".into(),
            Tok::Minus => "-".into(),
            Tok::Star => "*".into(),
            Tok::Slash => "/".into(),
            Tok::Caret => "^".into(),
            Tok::Percent => "%".into(),
            Tok::Bang => "!".into(),
            Tok::Square => "²".into(),
            Tok::Cube => "³".into(),
            Tok::LParen => "(".into(),
            Tok::RParen => ")".into(),
            Tok::Comma => ",".into(),
            Tok::Assign => "=".into(),
            Tok::Eof => "end of input".into(),
        }
    }
}

/// A token and its 1-based character position in the input.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub pos: usize,
}

/// Splits `input` into tokens; the last token is always [`Tok::Eof`].
pub fn lex(input: &str) -> Result<Vec<Token>, CalcError> {
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut out = Vec::new();
    let mut i = 0;

    while i < len {
        let c = chars[i];
        let pos = i + 1;

        if c.is_whitespace() {
            i += 1;
            continue;
        }

        let starts_number = c.is_ascii_digit()
            || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit()));
        if starts_number {
            let start = i;
            while i < len && chars[i].is_ascii_digit() {
                i += 1;
            }
            if i < len && chars[i] == '.' {
                i += 1;
                while i < len && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            // An exponent counts only if digits follow ("2e3", "1.5e-4");
            // otherwise the 'e' is the constant ("2e" means 2 * e).
            if i < len && (chars[i] == 'e' || chars[i] == 'E') {
                let mut j = i + 1;
                if j < len && (chars[j] == '+' || chars[j] == '-') {
                    j += 1;
                }
                if j < len && chars[j].is_ascii_digit() {
                    while j < len && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                    i = j;
                }
            }
            if i < len && chars[i] == '.' {
                return Err(CalcError::Syntax(format!(
                    "unexpected '.' at position {}",
                    i + 1
                )));
            }
            out.push(Token {
                tok: Tok::Number(chars[start..i].iter().collect()),
                pos,
            });
            continue;
        }

        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < len && (chars[i].is_alphabetic() || chars[i].is_ascii_digit() || chars[i] == '_')
            {
                i += 1;
            }
            let mut name: String = chars[start..i].iter().collect::<String>().to_lowercase();
            if name == "π" {
                name = "pi".into();
            }
            out.push(Token {
                tok: Tok::Ident(name),
                pos,
            });
            continue;
        }

        i += 1;
        let tok = match c {
            '+' => Tok::Plus,
            '-' | '−' | '–' => Tok::Minus,
            '*' => {
                if i < len && chars[i] == '*' {
                    i += 1;
                    Tok::Caret
                } else {
                    Tok::Star
                }
            }
            '×' | '·' => Tok::Star,
            '/' | '÷' => Tok::Slash,
            '^' => Tok::Caret,
            '%' => Tok::Percent,
            '!' => Tok::Bang,
            '²' => Tok::Square,
            '³' => Tok::Cube,
            '(' => Tok::LParen,
            ')' => Tok::RParen,
            ',' => Tok::Comma,
            '=' => Tok::Assign,
            '√' => Tok::Ident("sqrt".into()),
            other => {
                return Err(CalcError::Syntax(format!(
                    "unexpected character '{other}' at position {pos}"
                )))
            }
        };
        out.push(Token { tok, pos });
    }

    out.push(Token {
        tok: Tok::Eof,
        pos: len + 1,
    });
    Ok(out)
}
