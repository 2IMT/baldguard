use std::{error::Error, fmt::Display};

#[derive(Clone, Debug)]
pub struct GenericError {
    message: String,
}

impl From<String> for GenericError {
    fn from(message: String) -> Self {
        GenericError { message }
    }
}

impl Display for GenericError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for GenericError {}
