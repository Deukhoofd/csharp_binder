use crate::builder::{build_csharp, parse_script};
use std::fmt::Formatter;

mod builder;

#[cfg(test)]
mod tests;

pub struct CSharpBuilder {
    dll_name: String,
    tokens: syn::File,
    namespace: Option<String>,
    type_name: Option<String>,
}

impl CSharpBuilder {
    pub fn new(script: &str, dll_name: &str) -> Result<CSharpBuilder, Error> {
        match parse_script(script) {
            Ok(tokens) => Ok(CSharpBuilder {
                dll_name: dll_name.to_string(),
                tokens,
                namespace: None,
                type_name: None,
            }),
            Err(e) => Err(Error::from(e)),
        }
    }

    pub fn build(&self) -> Result<String, Error> {
        build_csharp(self)
    }

    pub fn set_namespace(&mut self, namespace: &str) {
        self.namespace = Some(namespace.to_string());
    }
    pub fn set_type(&mut self, type_name: &str) {
        self.type_name = Some(type_name.to_string());
    }
}

#[derive(Debug)]
pub enum Error {
    ParseError(syn::Error),
    IOError(std::io::Error),
    FmtError(std::fmt::Error),
    UnsupportedError(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ParseError(e) => e.fmt(f),
            Error::IOError(e) => e.fmt(f),
            Error::FmtError(e) => e.fmt(f),
            Error::UnsupportedError(e) => f.write_str(e),
        }
    }
}

impl From<syn::Error> for Error {
    fn from(error: syn::Error) -> Self {
        Error::ParseError(error)
    }
}
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::IOError(error)
    }
}
impl From<std::fmt::Error> for Error {
    fn from(error: std::fmt::Error) -> Self {
        Error::FmtError(error)
    }
}
