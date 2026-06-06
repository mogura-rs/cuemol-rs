use std::fmt;
use std::io;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CueMolError {
    InvalidArgument(String),
    ParseError(String),
    UndefinedReference(String),
    NotFound(String),
    Io(String),
}

pub type Result<T> = std::result::Result<T, CueMolError>;

impl CueMolError {
    pub fn parse(message: impl Into<String>) -> Self {
        Self::ParseError(message.into())
    }

    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::InvalidArgument(message.into())
    }

    pub fn undefined_reference(message: impl Into<String>) -> Self {
        Self::UndefinedReference(message.into())
    }

    pub fn message(&self) -> &str {
        match self {
            Self::InvalidArgument(message)
            | Self::ParseError(message)
            | Self::UndefinedReference(message)
            | Self::NotFound(message)
            | Self::Io(message) => message,
        }
    }
}

impl fmt::Display for CueMolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument(message) => write!(f, "invalid argument: {message}"),
            Self::ParseError(message) => write!(f, "syntax error: {message}"),
            Self::UndefinedReference(message) => write!(f, "undefined reference: {message}"),
            Self::NotFound(message) => write!(f, "not found: {message}"),
            Self::Io(message) => write!(f, "io error: {message}"),
        }
    }
}

impl std::error::Error for CueMolError {}

impl From<io::Error> for CueMolError {
    fn from(value: io::Error) -> Self {
        Self::Io(value.to_string())
    }
}
