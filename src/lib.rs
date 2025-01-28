use std::fmt::{Debug, Formatter};

pub enum Error {
    Reqwest(reqwest::Error),
    NotFound,
    StatusCode(reqwest::StatusCode),
    NoVersionsFound,
    InvalidIndex,
    NoFilesFound,
    Io(std::io::Error),
}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Self::Reqwest(value)
    }
}

impl From<reqwest::StatusCode> for Error {
    fn from(value: reqwest::StatusCode) -> Self {
        Self::StatusCode(value)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::Io(value)
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reqwest(arg0) => f.debug_tuple("Reqwest").field(arg0).finish(),
            Self::NotFound => write!(f, "Mod not found"),
            Self::StatusCode(arg0) => f.debug_tuple("StatusCode").field(arg0).finish(),
            Self::NoVersionsFound => write!(f, "No mod versions found"),
            Self::InvalidIndex => write!(f, "Invalid index"),
            Self::NoFilesFound => write!(f, "No files found"),
            Self::Io(arg0) => f.debug_tuple("IO").field(arg0).finish(),
        }
    }
}
