#[cfg(test)]
mod tests;
use std::borrow::Borrow;
use std::fmt::{Formatter, Write};
use syn::{
    Attribute, Expr, FnArg, Item, ItemEnum, ItemFn, ItemStruct, Meta, NestedMeta, Pat, ReturnType,
    Type,
};

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

fn parse_script(script: &str) -> syn::Result<syn::File> {
    syn::parse_str(script)
}

fn build_csharp(builder: &CSharpBuilder) -> Result<String, Error> {
    let mut script: String = "".to_string();
    let mut indent = 0;
    match &builder.namespace {
        None => {}
        Some(ns) => {
            write_line(&mut script, format!("namespace {}", ns), indent)?;
            write_line(&mut script, "{".to_string(), indent)?;
            indent += 1;
        }
    };
    match &builder.type_name {
        None => {}
        Some(t) => {
            write_line(&mut script, format!("internal static class {}", t), indent)?;
            write_line(&mut script, "{".to_string(), indent)?;
            indent += 1;
        }
    }

    for token in &builder.tokens.items {
        write_token(&mut script, token, &mut indent, builder)?;
    }

    match &builder.type_name {
        None => {}
        Some(_) => {
            indent -= 1;
            write_line(&mut script, "}".to_string(), indent)?;
        }
    }
    match &builder.namespace {
        None => {}
        Some(_) => {
            indent -= 1;
            write_line(&mut script, "}".to_string(), indent)?;
        }
    };
    Ok(script)
}

fn write_token(
    str: &mut String,
    token: &Item,
    indents: &mut i32,
    builder: &CSharpBuilder,
) -> Result<(), Error> {
    match token {
        Item::Const(_) => {}
        Item::Enum(en) => write_enum(str, indents, en)?,
        Item::ExternCrate(_) => {}
        Item::Fn(fun) => write_function(str, indents, builder, fun)?,
        Item::ForeignMod(_) => {}
        Item::Impl(_) => {}
        Item::Macro(_) => {}
        Item::Macro2(_) => {}
        Item::Mod(module) => {
            // We don't particularly care for the module itself (should we? Potentially make it a separate class?)
            // But we do care for the items inside, so extract those.
            match &module.content.as_ref() {
                None => {}
                Some(r) => {
                    for item in &r.1 {
                        write_token(str, item, indents, builder)?
                    }
                }
            }
        }
        Item::Static(_) => {}
        Item::Struct(strct) => write_struct(str, indents, strct)?,
        Item::Trait(_) => {}
        Item::TraitAlias(_) => {}
        Item::Type(_) => {}
        Item::Union(_) => {}
        Item::Use(_) => {}
        Item::Verbatim(_) => {}
        Item::__TestExhaustive(_) => {}
    }
    Ok(())
}

fn write_function(
    str: &mut String,
    indents: &mut i32,
    builder: &CSharpBuilder,
    fun: &ItemFn,
) -> Result<(), Error> {
    if !is_extern_c(fun) {
        return Ok(());
    }

    let return_type = match &fun.sig.output {
        ReturnType::Default => ("void".to_string(), "void".to_string()),
        ReturnType::Type(_, t) => convert_type_name(t.borrow()).expect("No name found"),
    };
    let mut parameters: Vec<(String, String, String)> = Vec::new();
    for input in &fun.sig.inputs {
        match input {
            FnArg::Receiver(_) => {
                return Err(Error::UnsupportedError(
                    "Receiver parameters aren't supported".to_string(),
                ))
            }
            FnArg::Typed(t) => match t.pat.borrow() {
                Pat::Ident(i) => {
                    let type_name = convert_type_name(t.ty.borrow())?;
                    parameters.push((
                        convert_naming(&i.ident.to_string(), true),
                        type_name.0,
                        type_name.1,
                    ));
                }
                _ => {
                    return Err(Error::UnsupportedError(
                        "Parameters that are not identity aren't supported".to_string(),
                    ))
                }
            },
        }
    }

    let outer_docs = extract_outer_docs(&fun.attrs)?;
    write_summary_from_outer_docs(str, outer_docs, indents)?;

    for parameter in &parameters {
        write_line(
            str,
            format!(
                "/// <param name=\"{}\">{}</param>",
                parameter.0, parameter.2
            ),
            *indents,
        )?;
    }
    write_line(
        str,
        format!("/// <returns>{}</returns>", return_type.1),
        *indents,
    )?;
    write_line(
        str,
        format!(
            "[DllImport(\"{}\", CallingConvention = CallingConvention.Cdecl, EntryPoint=\"{}\")]",
            builder.dll_name,
            fun.sig.ident.to_string()
        ),
        *indents,
    )?;

    for _ in 0..*indents {
        write!(str, "    ").ok();
    }
    write!(
        str,
        "internal static extern {} {}(",
        return_type.0,
        convert_naming(&fun.sig.ident.to_string(), false)
    )?;

    for (i, parameter) in parameters.iter().enumerate() {
        if i != 0 {
            write!(str, ", ")?;
        }
        write!(str, "{} {}", parameter.1, parameter.0)?;
    }
    writeln!(str, ");")?;
    Ok(())
}

fn write_enum(str: &mut String, indents: &mut i32, en: &ItemEnum) -> Result<(), Error> {
    let mut size_option: Option<(String, String)> = None;
    for attr in &en.attrs {
        let repr_attr = get_repr_attribute_value(attr)?;
        match repr_attr {
            None => {}
            Some(val) => {
                match val.get_ident() {
                    None => {}
                    Some(identifier) => {
                        match identifier.to_string().as_str() {
                            "C" => {
                                return Err(Error::UnsupportedError(
                                    "The size of a repr[C] enum is not specifically defined. Please use repr[u*] to define an actual size".to_string(),
                                ))
                            }
                            _ => size_option = Some(convert_type_path(&val)?),
                        }
                    }
                }
            }
        }
    }
    if size_option == None {
        return Ok(());
    }
    let size = size_option.expect("");

    let outer_docs = extract_outer_docs(&en.attrs)?;
    write_summary_from_outer_docs(str, outer_docs, indents)?;
    write_line(
        str,
        format!("public enum {} : {}", en.ident.to_string(), size.0),
        *indents,
    )?;
    write_line(str, "{".to_string(), *indents)?;
    *indents += 1;

    for variant in &en.variants {
        if !variant.fields.is_empty() {
            return Err(Error::UnsupportedError(
                "Enum with values with fields is not supported".to_string(),
            ));
        }

        let outer_docs = extract_outer_docs(&variant.attrs)?;
        write_summary_from_outer_docs(str, outer_docs, indents)?;

        let name = variant.ident.to_string();
        for _ in 0..*indents {
            write!(str, "    ").ok();
        }
        write!(str, "{}", name).ok();
        match &variant.discriminant {
            Some(v) => {
                let expr = v.1.borrow();
                if let Expr::Lit(l) = expr {
                    if let syn::Lit::Int(i) = &l.lit {
                        write!(str, " = {}", i.base10_digits()).ok();
                    }
                }
            }
            None => {}
        }

        write!(str, ",").ok();
        writeln!(str).ok();
    }

    *indents -= 1;
    write_line(str, "}".to_string(), *indents)?;
    Ok(())
}

fn write_struct(str: &mut String, indents: &mut i32, strct: &ItemStruct) -> Result<(), Error> {
    let mut found_c_repr = false;
    for attr in &strct.attrs {
        let repr_attr = get_repr_attribute_value(attr)?;
        match repr_attr {
            None => {}
            Some(val) => match &val.get_ident() {
                None => {}
                Some(attr_identifier) => {
                    if let "C" = attr_identifier.to_string().as_str() {
                        found_c_repr = true
                    }
                }
            },
        }
    }
    if !found_c_repr {
        return Ok(());
    }

    let outer_docs = extract_outer_docs(&strct.attrs)?;
    write_summary_from_outer_docs(str, outer_docs, indents)?;

    write_line(
        str,
        "[StructLayout(LayoutKind.Sequential, CharSet = CharSet.Ansi)]".to_string(),
        *indents,
    )?;
    write_line(
        str,
        format!("public struct {}", strct.ident.to_string()),
        *indents,
    )?;
    write_line(str, "{".to_string(), *indents)?;

    *indents += 1;
    for field in &strct.fields {
        let t = convert_type_name(&field.ty)?;
        let outer_docs = extract_outer_docs(&field.attrs)?;
        write_summary_from_outer_docs(str, outer_docs, indents)?;

        write_line(str, format!("/// <remarks>{}</remarks>", t.1), *indents)?;

        match &field.ident {
            None => {}
            Some(field_identifier) => {
                write_line(
                    str,
                    format!(
                        "public readonly {} {};",
                        t.0,
                        convert_naming(field_identifier.to_string().as_str(), false)
                    ),
                    *indents,
                )?;
            }
        }
    }
    *indents -= 1;

    write_line(str, "}".to_string(), *indents)?;
    Ok(())
}

fn extract_outer_docs(attrs: &[Attribute]) -> Result<Vec<String>, Error> {
    let mut outer_docs: Vec<String> = Vec::new();
    for attr in attrs {
        let parsed = attr.parse_meta()?;
        match parsed {
            Meta::Path(_) => {}
            Meta::List(_) => {}
            Meta::NameValue(nv) => match nv.path.get_ident() {
                None => {}
                Some(identifier) => {
                    if let "doc" = identifier.to_string().as_str() {
                        if let syn::Lit::Str(v) = nv.lit {
                            outer_docs.push(v.value());
                        }
                    }
                }
            },
        }
    }
    Ok(outer_docs)
}

fn write_summary_from_outer_docs(
    str: &mut String,
    outer_docs: Vec<String>,
    indents: &mut i32,
) -> Result<(), Error> {
    if !outer_docs.is_empty() {
        write_line(str, "/// <summary>".to_string(), *indents)?;
        for outer_doc in outer_docs {
            write_line(str, format!("/// {}", outer_doc.trim()), *indents)?;
        }
        write_line(str, "/// </summary>".to_string(), *indents)?;
    }
    Ok(())
}

fn is_extern_c(func: &ItemFn) -> bool {
    match &func.sig.abi {
        None => false,
        Some(abi) => match &abi.name {
            None => false,
            Some(name) => name.value() == "C",
        },
    }
}

fn convert_type_name(t: &syn::Type) -> Result<(String, String), Error> {
    match t {
        Type::Array(_) => Err(Error::UnsupportedError(
            "Using rust arrays from ffi is not supported.".to_string(),
        )),
        Type::BareFn(_) => Err(Error::UnsupportedError(
            "Using bare functions from ffi is not supported.".to_string(),
        )),
        Type::Group(_) => Err(Error::UnsupportedError(
            "Using type group from ffi is not supported.".to_string(),
        )),
        Type::ImplTrait(_) => Err(Error::UnsupportedError(
            "Using rust impl traits from ffi is not supported.".to_string(),
        )),
        Type::Infer(_) => Err(Error::UnsupportedError(
            "Using type infers is not supported. We can't generate a binding if we do not know the type.".to_string(),
        )),
        Type::Macro(_) => Err(Error::UnsupportedError(
            "Using rust macros from ffi is not supported.".to_string(),
        )),
        Type::Never(_) => Err(Error::UnsupportedError(
            "Using rust never type from ffi is not supported.".to_string(),
        )),
        Type::Paren(_) => Err(Error::UnsupportedError(
            "Using rust parenthesis from ffi is not supported.".to_string(),
        )),
        Type::Path(p) => convert_type_path(&p.path),
        Type::Ptr(ptr) => {
            let underlying = convert_type_name(ptr.elem.borrow())?;
            Ok(("IntPtr".to_string(), underlying.1 + "*"))
        }
        Type::Reference(r) => {
            let underlying = convert_type_name(r.elem.borrow())?;
            Ok((
                "ref ".to_string() + underlying.0.as_str(),
                underlying.1 + "&",
            ))
        }
        Type::Slice(_) => Err(Error::UnsupportedError(
            "Using rust slices from ffi is not supported.".to_string(),
        )),
        Type::TraitObject(_) => Err(Error::UnsupportedError(
            "Using rust traits from ffi is not supported.".to_string(),
        )),
        Type::Tuple(_) => Err(Error::UnsupportedError(
            "Using rust tuples from ffi is not supported.".to_string(),
        )),
        Type::Verbatim(_) => Err(Error::UnsupportedError(
            "Using rust verbatim from ffi is not supported.".to_string(),
        )),
        Type::__TestExhaustive(_) => unreachable!(),
    }
}

/// Convert Rust naming scheme (underscore snake_case) to C# naming scheme (CamelCase)
fn convert_naming(input: &str, is_parameter: bool) -> String {
    let mut split: Vec<String> = input.split('_').map(|x| x.to_string()).collect();
    for s in &mut split {
        if let Some(r) = s.get_mut(0..1) {
            r.make_ascii_uppercase();
        }
    }
    let mut f = split.join("");
    if is_parameter {
        if let Some(r) = f.get_mut(0..1) {
            r.make_ascii_lowercase();
        }
    }

    f
}

fn get_repr_attribute_value(attr: &Attribute) -> Result<Option<syn::Path>, Error> {
    let parsed = attr.parse_meta()?;
    match parsed {
        Meta::Path(_) => Ok(None),
        Meta::List(ls) => match ls.path.get_ident() {
            None => Ok(None),
            Some(identifier) => {
                if let "repr" = identifier.to_string().as_str() {
                    let value_option = ls.nested.first();
                    match value_option {
                        None => Ok(None),
                        Some(value) => match value {
                            NestedMeta::Meta(val) => {
                                if let Meta::Path(path) = val {
                                    return Ok(Some(path.clone()));
                                }
                                Ok(None)
                            }
                            NestedMeta::Lit(_) => Ok(None),
                        },
                    }
                } else {
                    Ok(None)
                }
            }
        },
        Meta::NameValue(_) => Ok(None),
    }
}

fn convert_type_path(path: &syn::Path) -> Result<(String, String), Error> {
    if path.segments.len() == 1 {
        return match path.segments.last() {
            Some(v) => {
                match v.ident.to_string().as_str() {
                    "u8" => Ok(("byte".to_string(), "u8".to_string())),
                    "u16" => Ok(("ushort".to_string(), "u16".to_string())),
                    "u32" => Ok(("uint".to_string(), "u32".to_string())),
                    "u64" => Ok(("ulong".to_string(), "u64".to_string())),
                    "u128" => Ok(("System.Numerics.BigInteger".to_string(), "u128".to_string())),
                    // Use new C# 9 native integer type for size, as it should be the same.
                    "usize" => Ok(("nuint".to_string(), "u128".to_string())),

                    "i8" => Ok(("sbyte".to_string(), "i8".to_string())),
                    "i16" => Ok(("short".to_string(), "i16".to_string())),
                    "i32" => Ok(("int".to_string(), "i32".to_string())),
                    "i64" => Ok(("long".to_string(), "i64".to_string())),
                    "i128" => Ok(("System.Numerics.BigInteger".to_string(), "i128".to_string())),
                    // Use new C# 9 native integer type for size, as it should be the same.
                    "isize" => Ok(("nint".to_string(), "u128".to_string())),

                    "f32" => Ok(("float".to_string(), "f32".to_string())),
                    "f64" => Ok(("double".to_string(), "f64".to_string())),

                    "char" => Ok(("char".to_string(), "char".to_string())),

                    "bool" => Err(Error::UnsupportedError("Found a boolean type. Due to differing sizes on different operating systems this is not supported for extern C functions.".to_string())),
                    "str" => Err(Error::UnsupportedError("Found a str type. This is not supported, please use a char pointer instead.".to_string())),

                    _ => Ok((v.ident.to_string(), v.ident.to_string())),
                }
            }
            None => Err(Error::UnsupportedError(
                "Types without a path are not supported".to_string(),
            )),
        };
    }
    Err(Error::UnsupportedError(
        "Types without a path are not supported".to_string(),
    ))
}

fn write_line(str: &mut String, content: String, indents: i32) -> Result<(), Error> {
    for _ in 0..indents {
        write!(str, "    ")?;
    }
    str.write_str(&content)?;
    writeln!(str)?;
    Ok(())
}
