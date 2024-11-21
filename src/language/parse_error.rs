use std::fmt::Display;

#[derive(Debug, Clone)]
pub enum ParseError {
    IntegerOverflow(String),
    InvalidEscapeSequence(String),
}

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::IntegerOverflow(value) => write!(f, "integer literal {value} is too big"),
            ParseError::InvalidEscapeSequence(value) => write!(
                f,
                "string literal \"{value}\" contains invalid escape sequence(s)"
            ),
        }
    }
}
