use crate::builder::{build_csharp, parse_script};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Formatter;

mod builder;

#[cfg(test)]
mod tests;

pub(crate) struct CSharpType {
    pub namespace: Option<String>,
    pub inside_type: Option<String>,
    pub real_type_name: String,
}

pub struct CSharpConfiguration {
    known_types: HashMap<String, CSharpType>,
}

impl Default for CSharpConfiguration {
    fn default() -> Self {
        Self::new()
    }
}

impl CSharpConfiguration {
    pub fn new() -> Self {
        Self {
            known_types: HashMap::new(),
        }
    }

    pub fn add_known_type(
        &mut self,
        rust_type_name: &str,
        csharp_namespace: Option<String>,
        csharp_inside_type: Option<String>,
        csharp_type_name: String,
    ) {
        self.known_types.insert(
            rust_type_name.to_string(),
            CSharpType {
                namespace: csharp_namespace,
                inside_type: csharp_inside_type,
                real_type_name: csharp_type_name,
            },
        );
    }

    pub(crate) fn get_known_type(&self, rust_type_name: &str) -> Option<&CSharpType> {
        self.known_types.get(rust_type_name)
    }
}

pub struct CSharpBuilder<'a> {
    configuration: RefCell<&'a mut CSharpConfiguration>,
    dll_name: String,
    tokens: syn::File,
    namespace: Option<String>,
    type_name: Option<String>,
}

impl<'a> CSharpBuilder<'a> {
    pub fn new(
        script: &str,
        dll_name: &str,
        configuration: &'a mut CSharpConfiguration,
    ) -> Result<CSharpBuilder<'a>, Error> {
        match parse_script(script) {
            Ok(tokens) => Ok(CSharpBuilder {
                configuration: RefCell::new(configuration),
                dll_name: dll_name.to_string(),
                tokens,
                namespace: None,
                type_name: None,
            }),
            Err(e) => Err(Error::from(e)),
        }
    }

    pub fn build(&mut self) -> Result<String, Error> {
        build_csharp(self)
    }

    pub fn set_namespace(&mut self, namespace: &str) {
        self.namespace = Some(namespace.to_string());
    }
    pub fn set_type(&mut self, type_name: &str) {
        self.type_name = Some(type_name.to_string());
    }

    pub(crate) fn add_known_type(&self, rust_type_name: &str, csharp_type_name: &str) {
        self.configuration.borrow_mut().add_known_type(
            rust_type_name,
            self.namespace.clone(),
            self.type_name.clone(),
            csharp_type_name.to_string(),
        );
    }
}

#[derive(Debug)]
pub enum Error {
    ParseError(syn::Error),
    IOError(std::io::Error),
    FmtError(std::fmt::Error),
    UnsupportedError(String),
    UnknownType(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ParseError(e) => e.fmt(f),
            Error::IOError(e) => e.fmt(f),
            Error::FmtError(e) => e.fmt(f),
            Error::UnsupportedError(e) => f.write_str(e),
            Error::UnknownType(e) => f.write_str(e),
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
