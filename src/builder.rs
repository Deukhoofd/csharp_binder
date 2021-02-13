use crate::{CSharpBuilder, Error};
use std::borrow::Borrow;
use std::collections::HashSet;
use std::fmt::Write;
use syn::spanned::Spanned;
use syn::{
    Attribute, Expr, FnArg, GenericArgument, GenericParam, Item, ItemEnum, ItemFn, ItemStruct,
    Meta, NestedMeta, Pat, Path, PathArguments, ReturnType, Type,
};

struct TypeNameContainer {
    csharp_name: String,
    rust_name: String,
    generics: Vec<TypeNameContainer>,
}

impl TypeNameContainer {
    fn new(csharp_name: String, rust_name: String) -> TypeNameContainer {
        TypeNameContainer {
            csharp_name,
            rust_name,
            generics: Vec::new(),
        }
    }

    fn stringify(&self) -> Result<String, Error> {
        let mut s = self.csharp_name.to_string();
        if !self.generics.is_empty() {
            write!(s, "<")?;

            for (index, generic) in self.generics.iter().enumerate() {
                if index != 0 {
                    write!(s, ", ")?;
                }
                write!(s, "{}", generic.stringify()?)?;
            }

            write!(s, ">")?;
        }
        Ok(s)
    }
}

pub fn parse_script(script: &str) -> syn::Result<syn::File> {
    syn::parse_str(script)
}

pub fn build_csharp(builder: &CSharpBuilder) -> Result<String, Error> {
    let mut script: String = "".to_string();
    let mut indent = 0;

    {
        let generated_warning = &builder.configuration.borrow().generated_warning;
        if !generated_warning.is_empty() {
            for line in generated_warning.lines() {
                write_line(&mut script, "// ".to_string() + line, indent)?;
            }
        }
    }
    for using in &builder.usings {
        write_line(&mut script, format!("using {};", using), indent)?;
    }
    writeln!(script)?;

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
        Item::Enum(en) => write_enum(str, indents, en, builder)?,
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
        Item::Struct(strct) => write_struct(str, indents, strct, builder)?,
        Item::Trait(_) => {}
        Item::TraitAlias(_) => {}
        Item::Type(typedef) => {
            let ty: &Type = typedef.ty.borrow();
            if let Type::Path(type_path) = ty {
                let type_name_opt = get_path_name(&type_path.path);
                match type_name_opt {
                    None => {}
                    Some(type_name) => {
                        let mut conf = builder.configuration.borrow_mut();
                        let t = conf.get_known_type(type_name.as_str());
                        if t.is_none() {
                            return Ok(());
                        }
                        let inner_type = t.unwrap();
                        let namespace = inner_type.namespace.clone();
                        let inside_type = inner_type.inside_type.clone();
                        let mut real_type_name = inner_type.real_type_name.clone();

                        if let PathArguments::AngleBracketed(generics) =
                            &type_path.path.segments.last().unwrap().arguments
                        {
                            write!(real_type_name, "<")?;
                            for (index, generic) in generics.args.iter().enumerate() {
                                if let GenericArgument::Type(t) = generic {
                                    if index != 0 {
                                        write!(real_type_name, ", ")?;
                                    }
                                    write!(
                                        real_type_name,
                                        "{}",
                                        convert_type_name(t, builder)?.csharp_name
                                    )?;
                                }
                            }
                            write!(real_type_name, ">")?;
                        }

                        conf.add_known_type(
                            typedef.ident.to_string().as_str(),
                            namespace,
                            inside_type,
                            real_type_name,
                        )
                    }
                }
            }
        }
        Item::Union(_) => {}
        Item::Use(_) => {}
        Item::Verbatim(_) => {}
        Item::__TestExhaustive(_) => {}
    }
    Ok(())
}

fn get_path_name(path: &Path) -> Option<String> {
    Some(path.segments.last()?.ident.to_string())
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
        ReturnType::Default => TypeNameContainer::new("void".to_string(), "void".to_string()),
        ReturnType::Type(_, t) => convert_type_name(t.borrow(), builder)?,
    };
    let mut parameters: Vec<(String, String, String)> = Vec::new();
    for input in &fun.sig.inputs {
        match input {
            FnArg::Receiver(_) => {
                return Err(Error::UnsupportedError(
                    "Receiver parameters aren't supported".to_string(),
                    input.span(),
                ))
            }
            FnArg::Typed(t) => match t.pat.borrow() {
                Pat::Ident(i) => {
                    let type_name = convert_type_name(t.ty.borrow(), builder)?;
                    parameters.push((
                        convert_naming(&i.ident.to_string(), true),
                        type_name.stringify()?,
                        type_name.rust_name,
                    ));
                }
                _ => {
                    return Err(Error::UnsupportedError(
                        "Parameters that are not identity aren't supported".to_string(),
                        input.span(),
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
        format!("/// <returns>{}</returns>", return_type.rust_name),
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
        return_type.stringify()?,
        convert_naming(&fun.sig.ident.to_string(), false)
    )?;

    for (i, parameter) in parameters.iter().enumerate() {
        if i != 0 {
            write!(str, ", ")?;
        }
        write!(str, "{} {}", parameter.1, parameter.0)?;
    }
    writeln!(str, ");")?;
    writeln!(str)?;

    Ok(())
}

fn write_enum(
    str: &mut String,
    indents: &mut i32,
    en: &ItemEnum,
    builder: &CSharpBuilder,
) -> Result<(), Error> {
    let mut size_option: Option<TypeNameContainer> = None;
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
                                    identifier.span()
                                ))
                            }
                            _ => size_option = Some(convert_type_path(&val, builder)?),
                        }
                    }
                }
            }
        }
    }
    if size_option.is_none() {
        return Ok(());
    }
    let size = size_option.expect("");

    let outer_docs = extract_outer_docs(&en.attrs)?;
    write_summary_from_outer_docs(str, outer_docs, indents)?;
    write_line(
        str,
        format!(
            "public enum {} : {}",
            en.ident.to_string(),
            size.csharp_name
        ),
        *indents,
    )?;
    write_line(str, "{".to_string(), *indents)?;
    *indents += 1;

    for variant in &en.variants {
        if !variant.fields.is_empty() {
            return Err(Error::UnsupportedError(
                "Enum with values with fields is not supported".to_string(),
                variant.span(),
            ));
        }

        let outer_docs = extract_outer_docs(&variant.attrs)?;
        write_summary_from_outer_docs(str, outer_docs, indents)?;

        let name = variant.ident.to_string();
        for _ in 0..*indents {
            write!(str, "    ")?;
        }
        write!(str, "{}", name)?;
        match &variant.discriminant {
            Some(v) => {
                let expr = v.1.borrow();
                if let Expr::Lit(l) = expr {
                    if let syn::Lit::Int(i) = &l.lit {
                        write!(str, " = {}", i.base10_digits())?;
                    }
                }
            }
            None => {}
        }

        write!(str, ",")?;
        writeln!(str)?;
    }
    *indents -= 1;
    write_line(str, "}".to_string(), *indents)?;
    writeln!(str)?;

    builder.add_known_type(en.ident.to_string().as_str(), en.ident.to_string().as_str());
    Ok(())
}

fn write_struct(
    str: &mut String,
    indents: &mut i32,
    strct: &ItemStruct,
    builder: &CSharpBuilder,
) -> Result<(), Error> {
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
        "[StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]".to_string(),
        *indents,
    )?;

    for _ in 0..*indents {
        write!(str, "    ")?;
    }
    write!(str, "public struct {}", strct.ident.to_string())?;

    let mut generics: HashSet<String> = HashSet::new();
    for param in &strct.generics.params {
        match param {
            GenericParam::Type(type_param) => {
                generics.insert(type_param.ident.to_string());
            }
            GenericParam::Lifetime(_) => {}
            GenericParam::Const(_) => {}
        }
    }

    if !generics.is_empty() {
        write!(str, "<")?;

        for (index, generic) in generics.iter().enumerate() {
            if index != 0 {
                write!(str, ", ")?;
            }
            write!(str, "{}", generic)?;
        }

        write!(str, ">")?;
    }

    writeln!(str)?;
    write_line(str, "{".to_string(), *indents)?;

    *indents += 1;
    let mut converted_fields: Vec<(String, String)> = Vec::new();

    for field in &strct.fields {
        let mut generic_t = None;
        if let Type::Path(p) = &field.ty {
            match p.path.get_ident() {
                None => {}
                Some(ident) => {
                    if generics.contains(ident.to_string().as_str()) {
                        generic_t = Some(ident.to_string())
                    }
                }
            }
        }

        let t = match generic_t {
            None => convert_type_name(&field.ty, builder)?,
            Some(v) => TypeNameContainer::new(v.to_string(), v),
        };
        let outer_docs = extract_outer_docs(&field.attrs)?;
        write_summary_from_outer_docs(str, outer_docs, indents)?;

        write_line(
            str,
            format!("/// <remarks>{}</remarks>", t.rust_name),
            *indents,
        )?;

        match &field.ident {
            None => {}
            Some(field_identifier) => {
                let csharp_field_name =
                    convert_naming(field_identifier.to_string().as_str(), false);
                // If C# version is 9 or newer, we make all fields { get; init; }, so they can be
                // initialised, but are readonly afterwards. Otherwise we just make them readonly.
                if builder.configuration.borrow().csharp_version >= 9 {
                    write_line(
                        str,
                        format!(
                            "public {} {} {{ get; init; }}",
                            t.stringify()?,
                            csharp_field_name
                        ),
                        *indents,
                    )?;
                } else {
                    write_line(
                        str,
                        format!("public readonly {} {};", t.stringify()?, csharp_field_name),
                        *indents,
                    )?;
                }
                converted_fields.push((t.stringify()?, csharp_field_name));
            }
        }
    }

    writeln!(str)?;

    for _ in 0..*indents {
        write!(str, "    ")?;
    }
    write!(str, "public {}(", strct.ident.to_string())?;
    for (index, converted_field) in converted_fields.iter().enumerate() {
        if index != 0 {
            write!(str, ", ")?;
        }

        let mut parameter_name = converted_field.1.to_string();
        if let Some(r) = parameter_name.get_mut(0..1) {
            r.make_ascii_lowercase();
        }

        write!(str, "{} {}", converted_field.0, parameter_name)?;
    }
    writeln!(str, ")")?;
    write_line(str, "{".to_string(), *indents)?;
    *indents += 1;

    for converted_field in converted_fields {
        let mut parameter_name = converted_field.1.to_string();
        if let Some(r) = parameter_name.get_mut(0..1) {
            r.make_ascii_lowercase();
        }
        write_line(
            str,
            format!("{} = {};", converted_field.1, parameter_name),
            *indents,
        )?;
    }
    *indents -= 1;

    write_line(str, "}".to_string(), *indents)?;

    *indents -= 1;
    write_line(str, "}".to_string(), *indents)?;
    writeln!(str)?;

    builder.add_known_type(
        strct.ident.to_string().as_str(),
        strct.ident.to_string().as_str(),
    );
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

fn convert_type_name(t: &syn::Type, builder: &CSharpBuilder) -> Result<TypeNameContainer, Error> {
    match t {
        Type::Array(_) => Err(Error::UnsupportedError(
            "Using rust arrays from ffi is not supported.".to_string(),
            t.span()
        )),
        Type::BareFn(_) => Err(Error::UnsupportedError(
            "Using bare functions from ffi is not supported.".to_string(),
            t.span()
        )),
        Type::Group(_) => Err(Error::UnsupportedError(
            "Using type group from ffi is not supported.".to_string(),           
            t.span()
        )),
        Type::ImplTrait(_) => Err(Error::UnsupportedError(
            "Using rust impl traits from ffi is not supported.".to_string(),
            t.span()
        )),
        Type::Infer(_) => Err(Error::UnsupportedError(
            "Using type infers is not supported. We can't generate a binding if we do not know the type.".to_string(),
            t.span()
        )),
        Type::Macro(_) => Err(Error::UnsupportedError(
            "Using rust macros from ffi is not supported.".to_string(),
            t.span()
        )),
        Type::Never(_) => Err(Error::UnsupportedError(
            "Using rust never type from ffi is not supported.".to_string(),
            t.span()
        )),
        Type::Paren(_) => Err(Error::UnsupportedError(
            "Using rust parenthesis from ffi is not supported.".to_string(),
            t.span()
        )),
        Type::Path(p) => convert_type_path(&p.path, builder),
        Type::Ptr(ptr) => {
            let underlying = convert_type_name(ptr.elem.borrow(), builder)?;
            Ok(TypeNameContainer::new("IntPtr".to_string(), underlying.rust_name + "*"))
        }
        Type::Reference(r) => {
            let underlying = convert_type_name(r.elem.borrow(), builder)?;
            Ok(TypeNameContainer::new(
                "ref ".to_string() + underlying.stringify()?.as_str(),
                underlying.rust_name + "&",
            ))
        }
        Type::Slice(_) => Err(Error::UnsupportedError(
            "Using rust slices from ffi is not supported.".to_string(),            
            t.span()
        )),
        Type::TraitObject(_) => Err(Error::UnsupportedError(
            "Using rust traits from ffi is not supported.".to_string(),
            t.span()
        )),
        Type::Tuple(_) => Err(Error::UnsupportedError(
            "Using rust tuples from ffi is not supported.".to_string(),
            t.span()

        )),
        Type::Verbatim(_) => Err(Error::UnsupportedError(
            "Using rust verbatim from ffi is not supported.".to_string(),
            t.span()

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

fn convert_type_path(
    path: &syn::Path,
    builder: &CSharpBuilder,
) -> Result<TypeNameContainer, Error> {
    return match path.segments.last() {
        Some(v) => {
            match v.ident.to_string().as_str() {
                // First attempt to resolve the primitive types
                "u8" => Ok(TypeNameContainer::new("byte".to_string(), "u8".to_string())),
                "u16" => Ok(TypeNameContainer::new("ushort".to_string(), "u16".to_string())),
                "u32" => Ok(TypeNameContainer::new("uint".to_string(), "u32".to_string())),
                "u64" => Ok(TypeNameContainer::new("ulong".to_string(), "u64".to_string())),
                "u128" => Ok(TypeNameContainer::new("System.Numerics.BigInteger".to_string(), "u128".to_string())),
                "usize" => {
                    if builder.configuration.borrow().csharp_version >= 9 {
                        // Use new C# 9 native integer type for size, as it should be the same.
                        Ok(TypeNameContainer::new("nuint".to_string(), "usize".to_string()))
                    }
                    else{
                        // FIXME: Not strictly correct on 32 bit computers. 
                        Ok(TypeNameContainer::new("ulong".to_string(), "usize".to_string()))
                    }
                },

                "i8" => Ok(TypeNameContainer::new("sbyte".to_string(), "i8".to_string())),
                "i16" => Ok(TypeNameContainer::new("short".to_string(), "i16".to_string())),
                "i32" => Ok(TypeNameContainer::new("int".to_string(), "i32".to_string())),
                "i64" => Ok(TypeNameContainer::new("long".to_string(), "i64".to_string())),
                "i128" => Ok(TypeNameContainer::new("System.Numerics.BigInteger".to_string(), "i128".to_string())),
                "isize" => {
                    if builder.configuration.borrow().csharp_version >= 9 {
                        // Use new C# 9 native integer type for size, as it should be the same.
                        Ok(TypeNameContainer::new("nint".to_string(), "isize".to_string()))
                    }
                    else{
                        // FIXME: Not strictly correct on 32 bit computers. 
                        Ok(TypeNameContainer::new("long".to_string(), "isize".to_string()))
                    }
                },

                "f32" => Ok(TypeNameContainer::new("float".to_string(), "f32".to_string())),
                "f64" => Ok(TypeNameContainer::new("double".to_string(), "f64".to_string())),

                "char" => Ok(TypeNameContainer::new("char".to_string(), "char".to_string())),
                "c_char" => Ok(TypeNameContainer::new("char".to_string(), "c_char".to_string())),

                "bool" => Err(Error::UnsupportedError("Found a boolean type. Due to differing sizes on different operating systems this is not supported for extern C functions.".to_string(),             v.ident.span()
                )),
                "str" => Err(Error::UnsupportedError("Found a str type. This is not supported, please use a char pointer instead.".to_string(), v.ident.span())),

                // If the type is not a primitive type, attempt to resolve the type from our type database.
                _ => {
                    if builder.configuration.borrow().out_type.is_some() &&
                        &v.ident.to_string() == builder.configuration.borrow().out_type.as_ref().unwrap() {
                        return extract_out_parameter_type(v, builder);
                    }
                    let mut base = resolve_known_type_name(&builder, &v.ident)?;
                    if let PathArguments::AngleBracketed(generics) = &v.arguments {
                        for generic in &generics.args {
                            if let GenericArgument::Type(gen) = generic {
                                base.generics.push(convert_type_name(gen, builder)?)
                            }
                        }
                    }

                    Ok(base)
                },
            }
        }
        None => Err(Error::UnsupportedError(
            "Types without a path are not supported".to_string(),
            path.span(),
        )),
    };
}

fn extract_out_parameter_type(
    v: &syn::PathSegment,
    builder: &CSharpBuilder,
) -> Result<TypeNameContainer, Error> {
    return match &v.arguments {
        PathArguments::AngleBracketed(a) => match a.args.last() {
            Some(GenericArgument::Type(t)) => {
                let inner_type = convert_type_name(t, builder)?;
                Ok(TypeNameContainer::new(
                    "out ".to_string() + inner_type.stringify()?.as_str(),
                    v.ident.to_string(),
                ))
            }
            _ => Err(Error::UnsupportedError(
                "Out type requires the real type to be angle bracketed.".to_string(),
                v.ident.span(),
            )),
        },
        _ => Err(Error::UnsupportedError(
            "Out type requires the real type to be angle bracketed.".to_string(),
            v.ident.span(),
        )),
    };
}

fn resolve_known_type_name(
    builder: &CSharpBuilder,
    v: &syn::Ident,
) -> Result<TypeNameContainer, Error> {
    let conf = builder.configuration.borrow();
    let t = conf.get_known_type(v.to_string().as_str());
    match t {
        None => Err(Error::UnknownType(
            format!("Type with name '{}' was not found", v.to_string()),
            v.span(),
        )),
        Some(t) => {
            let inside_type = &builder.type_name;
            if builder.namespace == t.namespace
                && (*inside_type == t.inside_type || t.inside_type.is_none())
            {
                Ok(TypeNameContainer::new(
                    t.real_type_name.to_string(),
                    v.to_string(),
                ))
            } else if builder.namespace == t.namespace {
                Ok(TypeNameContainer::new(
                    t.inside_type.as_ref().unwrap().to_string()
                        + "."
                        + &*t.real_type_name.to_string(),
                    v.to_string(),
                ))
            } else if t.inside_type.is_none() {
                if t.namespace.is_none() {
                    Ok(TypeNameContainer::new(
                        t.real_type_name.to_string(),
                        v.to_string(),
                    ))
                } else {
                    Ok(TypeNameContainer::new(
                        t.namespace.as_ref().unwrap().to_string()
                            + "."
                            + &*t.real_type_name.to_string(),
                        v.to_string(),
                    ))
                }
            } else if t.namespace.is_none() {
                Ok(TypeNameContainer::new(
                    t.inside_type.as_ref().unwrap().to_string()
                        + "."
                        + t.real_type_name.to_string().as_str(),
                    v.to_string(),
                ))
            } else {
                Ok(TypeNameContainer::new(
                    t.namespace.as_ref().unwrap().to_string()
                        + "."
                        + t.inside_type.as_ref().unwrap().to_string().as_str()
                        + "."
                        + t.real_type_name.to_string().as_str(),
                    v.to_string(),
                ))
            }
        }
    }
}

fn write_line(str: &mut String, content: String, indents: i32) -> Result<(), Error> {
    for _ in 0..indents {
        write!(str, "    ")?;
    }
    str.write_str(&content)?;
    writeln!(str)?;
    Ok(())
}
