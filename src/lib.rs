use std::borrow::Borrow;
use std::fmt::Write;
use syn::{
    Attribute, Error, Expr, FnArg, Item, ItemEnum, ItemFn, ItemStruct, Meta, NestedMeta, Pat,
    ReturnType, Type,
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
            Err(e) => Err(e),
        }
    }

    pub fn build(&self) -> String {
        build_csharp(self)
    }

    pub fn set_namespace(&mut self, namespace: &str) {
        self.namespace = Some(namespace.to_string());
    }
    pub fn set_type(&mut self, type_name: &str) {
        self.type_name = Some(type_name.to_string());
    }
}

fn parse_script(script: &str) -> syn::Result<syn::File> {
    syn::parse_str(script)
}

fn build_csharp(builder: &CSharpBuilder) -> String {
    let mut script: String = "".to_string();
    let mut indent = 0;
    match &builder.namespace {
        None => {}
        Some(ns) => {
            write_line(&mut script, format!("namespace {}", ns), indent);
            write_line(&mut script, "{".to_string(), indent);
            indent += 1;
        }
    };
    match &builder.type_name {
        None => {}
        Some(t) => {
            write_line(&mut script, format!("internal static class {}", t), indent);
            write_line(&mut script, "{".to_string(), indent);
            indent += 1;
        }
    }

    for token in &builder.tokens.items {
        write_token(&mut script, token, &mut indent, builder);
    }

    match &builder.type_name {
        None => {}
        Some(_) => {
            indent -= 1;
            write_line(&mut script, "}".to_string(), indent);
        }
    }
    match &builder.namespace {
        None => {}
        Some(_) => {
            indent -= 1;
            write_line(&mut script, "}".to_string(), indent);
        }
    };
    script
}

fn write_token(str: &mut String, token: &Item, indents: &mut i32, builder: &CSharpBuilder) {
    match token {
        Item::Const(_) => {}
        Item::Enum(en) => write_enum(str, indents, en),
        Item::ExternCrate(_) => {}
        Item::Fn(fun) => write_function(str, indents, builder, fun),
        Item::ForeignMod(_) => {}
        Item::Impl(_) => {}
        Item::Macro(_) => {}
        Item::Macro2(_) => {}
        Item::Mod(module) => {
            // We don't particularly care for the module itself (should we? Potentially make it a separate class?)
            // But we do care for the items inside, so extract those.
            for item in &module.content.as_ref().unwrap().1 {
                write_token(str, item, indents, builder);
            }
        }
        Item::Static(_) => {}
        Item::Struct(strct) => write_struct(str, indents, strct),
        Item::Trait(_) => {}
        Item::TraitAlias(_) => {}
        Item::Type(_) => {}
        Item::Union(_) => {}
        Item::Use(_) => {}
        Item::Verbatim(_) => {}
        Item::__TestExhaustive(_) => {}
    }
}

fn write_function(str: &mut String, indents: &mut i32, builder: &CSharpBuilder, fun: &ItemFn) {
    if !is_extern_c(fun) {
        return;
    }

    let return_type = match &fun.sig.output {
        ReturnType::Default => ("void".to_string(), "void".to_string()),
        ReturnType::Type(_, t) => convert_type_name(t.borrow()).unwrap(),
    };
    let mut parameters: Vec<(String, String, String)> = Vec::new();
    for input in &fun.sig.inputs {
        match input {
            FnArg::Receiver(_) => unimplemented!(),
            FnArg::Typed(t) => match t.pat.borrow() {
                Pat::Ident(i) => {
                    let type_name = convert_type_name(t.ty.borrow()).unwrap();
                    parameters.push((
                        convert_naming(&i.ident.to_string(), true),
                        type_name.0,
                        type_name.1,
                    ));
                }
                _ => unimplemented!(),
            },
        }
    }

    let outer_docs = extract_outer_docs(&fun.attrs);
    write_summary_from_outer_docs(str, outer_docs, indents);

    for parameter in &parameters {
        write_line(
            str,
            format!(
                "/// <param name=\"{}\">{}</param>",
                parameter.0, parameter.2
            ),
            *indents,
        );
    }
    write_line(
        str,
        format!("/// <returns>{}</returns>", return_type.1),
        *indents,
    );
    write_line(
        str,
        format!(
            "[DllImport(\"{}\", CallingConvention = CallingConvention.Cdecl, EntryPoint=\"{}\")]",
            builder.dll_name,
            fun.sig.ident.to_string()
        ),
        *indents,
    );

    for _ in 0..*indents {
        write!(str, "    ").ok();
    }
    write!(
        str,
        "internal static extern {} {}(",
        return_type.0,
        convert_naming(&fun.sig.ident.to_string(), false)
    )
    .ok();

    for (i, parameter) in parameters.iter().enumerate() {
        if i != 0 {
            write!(str, ", ").unwrap();
        }
        write!(str, "{} {}", parameter.1, parameter.0).unwrap();
    }
    writeln!(str, ");").ok();
}

fn write_enum(str: &mut String, indents: &mut i32, en: &ItemEnum) {
    let mut size: Option<(String, String)> = None;
    for attr in &en.attrs {
        let repr_attr = get_repr_attribute_value(attr);
        match repr_attr {
            None => {}
            Some(val) => {
                match val.get_ident().unwrap().to_string().as_str() {
                    "C" => panic!("The size of a repr[C] enum is not specifically defined. Please use repr[u*] to define an actual size."),
                    _ => size = convert_type_path(&val),
                }
            }
        }
    }
    if size == None {
        return;
    }

    let outer_docs = extract_outer_docs(&en.attrs);
    write_summary_from_outer_docs(str, outer_docs, indents);
    write_line(
        str,
        format!("public enum {} : {}", en.ident.to_string(), size.unwrap().0),
        *indents,
    );
    write_line(str, "{".to_string(), *indents);
    *indents += 1;

    for variant in &en.variants {
        if !variant.fields.is_empty() {
            panic!("Enum with values with fields is not supported");
        }

        let outer_docs = extract_outer_docs(&variant.attrs);
        write_summary_from_outer_docs(str, outer_docs, indents);

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
    write_line(str, "}".to_string(), *indents);
}

fn write_struct(str: &mut String, indents: &mut i32, strct: &ItemStruct) {
    let mut found_c_repr = false;
    for attr in &strct.attrs {
        let repr_attr = get_repr_attribute_value(attr);
        match repr_attr {
            None => {}
            Some(val) => {
                if let "C" = val.get_ident().unwrap().to_string().as_str() {
                    found_c_repr = true
                }
            }
        }
    }
    if !found_c_repr {
        return;
    }

    let outer_docs = extract_outer_docs(&strct.attrs);
    write_summary_from_outer_docs(str, outer_docs, indents);

    write_line(
        str,
        "[StructLayout(LayoutKind.Sequential, CharSet = CharSet.Ansi)]".to_string(),
        *indents,
    );
    write_line(
        str,
        format!("public struct {}", strct.ident.to_string()),
        *indents,
    );
    write_line(str, "{".to_string(), *indents);

    *indents += 1;
    for field in &strct.fields {
        let t = convert_type_name(&field.ty).unwrap();

        let outer_docs = extract_outer_docs(&field.attrs);
        write_summary_from_outer_docs(str, outer_docs, indents);

        write_line(str, format!("/// <remarks>{}</remarks>", t.1), *indents);

        write_line(
            str,
            format!(
                "public readonly {} {};",
                t.0,
                convert_naming(&field.ident.as_ref().unwrap().to_string(), false)
            ),
            *indents,
        );
    }
    *indents -= 1;

    write_line(str, "}".to_string(), *indents);
}

fn extract_outer_docs(attrs: &[Attribute]) -> Vec<String> {
    let mut outer_docs: Vec<String> = Vec::new();
    for attr in attrs {
        let parsed = attr.parse_meta().unwrap();
        match parsed {
            Meta::Path(_) => {}
            Meta::List(_) => {}
            Meta::NameValue(nv) => {
                if let "doc" = nv.path.get_ident().unwrap().to_string().as_str() {
                    if let syn::Lit::Str(v) = nv.lit {
                        outer_docs.push(v.value());
                    }
                }
            }
        }
    }
    outer_docs
}

fn write_summary_from_outer_docs(str: &mut String, outer_docs: Vec<String>, indents: &mut i32) {
    if !outer_docs.is_empty() {
        write_line(str, "/// <summary>".to_string(), *indents);
        for outer_doc in outer_docs {
            write_line(str, format!("/// {}", outer_doc.trim()), *indents);
        }
        write_line(str, "/// </summary>".to_string(), *indents);
    }
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

fn convert_type_name(t: &syn::Type) -> Option<(String, String)> {
    match t {
        Type::Array(_) => unimplemented!(),
        Type::BareFn(_) => unimplemented!(),
        Type::Group(_) => unimplemented!(),
        Type::ImplTrait(_) => unimplemented!(),
        Type::Infer(_) => unimplemented!(),
        Type::Macro(_) => unimplemented!(),
        Type::Never(_) => unimplemented!(),
        Type::Paren(_) => unimplemented!(),
        Type::Path(p) => convert_type_path(&p.path),
        Type::Ptr(ptr) => {
            let underlying = convert_type_name(ptr.elem.borrow()).unwrap();
            Some(("IntPtr".to_string(), underlying.1 + "*"))
        }
        Type::Reference(r) => {
            let underlying = convert_type_name(r.elem.borrow()).unwrap();
            Some((
                "ref ".to_string() + underlying.0.as_str(),
                underlying.1 + "&",
            ))
        }
        Type::Slice(_) => unimplemented!(),
        Type::TraitObject(_) => unimplemented!(),
        Type::Tuple(_) => unimplemented!(),
        Type::Verbatim(_) => unimplemented!(),
        Type::__TestExhaustive(_) => unimplemented!(),
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

fn get_repr_attribute_value(attr: &Attribute) -> Option<syn::Path> {
    let parsed = attr.parse_meta().unwrap();
    match parsed {
        Meta::Path(_) => None,
        Meta::List(ls) => {
            if let "repr" = ls.path.get_ident().unwrap().to_string().as_str() {
                let value = ls.nested.first().unwrap();
                match value {
                    NestedMeta::Meta(val) => {
                        if let Meta::Path(path) = val {
                            return Some(path.clone());
                        }
                        None
                    }
                    NestedMeta::Lit(_) => None,
                }
            } else {
                None
            }
        }
        Meta::NameValue(_) => None,
    }
}

fn convert_type_path(path: &syn::Path) -> Option<(String, String)> {
    if path.segments.len() == 1 {
        match path.segments.last() {
            Some(v) => {
                return match v.ident.to_string().as_str() {
                    "u8" => Some(("byte".to_string(), "u8".to_string())),
                    "u16" => Some(("ushort".to_string(), "u16".to_string())),
                    "u32" => Some(("uint".to_string(), "u32".to_string())),
                    "u64" => Some(("ulong".to_string(), "u64".to_string())),
                    "u128" => Some(("System.Numerics.BigInteger".to_string(), "u128".to_string())),
                    // Use new C# 9 native integer type for size, as it should be the same.
                    "usize" => Some(("nuint".to_string(), "u128".to_string())),

                    "i8" => Some(("sbyte".to_string(), "i8".to_string())),
                    "i16" => Some(("short".to_string(), "i16".to_string())),
                    "i32" => Some(("int".to_string(), "i32".to_string())),
                    "i64" => Some(("long".to_string(), "i64".to_string())),
                    "i128" => Some(("System.Numerics.BigInteger".to_string(), "i128".to_string())),
                    // Use new C# 9 native integer type for size, as it should be the same.
                    "isize" => Some(("nint".to_string(), "u128".to_string())),

                    "f32" => Some(("float".to_string(), "f32".to_string())),
                    "f64" => Some(("double".to_string(), "f64".to_string())),

                    "char" => Some(("char".to_string(), "char".to_string())),

                    "bool" => panic!("Found a boolean type. Due to differing sizes on different operating systems this is not supported for extern C functions."),
                    "str" => panic!("Found a str type. This is not supported, please use a char pointer instead."),

                    _ => Some((v.ident.to_string(), v.ident.to_string())),
                };
            }
            None => unimplemented!(),
        }
    }
    unimplemented!()
}

fn write_line(str: &mut String, content: String, indents: i32) {
    for _ in 0..indents {
        write!(str, "    ").ok();
    }
    str.write_str(&content).ok();
    writeln!(str).ok();
}

#[cfg(test)]
mod tests {
    use crate::CSharpBuilder;

    #[test]
    fn create_builder() {
        CSharpBuilder::new(r#"pub fn foo(){}"#, "foo").unwrap();
    }
    #[test]
    fn build_empty_with_namespace() {
        let mut builder = CSharpBuilder::new(r#""#, "foo").unwrap();
        builder.set_namespace("foo");
        let script = builder.build();
        assert_eq!(script, "namespace foo\n{\n}\n")
    }

    #[test]
    fn build_empty_with_type() {
        let mut builder = CSharpBuilder::new(r#""#, "foo").unwrap();
        builder.set_type("foo");
        let script = builder.build();
        assert_eq!(script, "internal static class foo\n{\n}\n")
    }

    #[test]
    fn build_empty_with_namespace_and_type() {
        let mut builder = CSharpBuilder::new(r#""#, "foo").unwrap();
        builder.set_namespace("foo");
        builder.set_type("bar");
        let script = builder.build();
        assert_eq!(
            script,
            "namespace foo
{
    internal static class bar
    {
    }
}\n"
        )
    }

    #[test]
    fn build_with_void_function() {
        let mut builder = CSharpBuilder::new(r#"pub extern "C" fn foo(){}"#, "foo").unwrap();
        builder.set_namespace("foo");
        builder.set_type("bar");
        let script = builder.build();
        assert_eq!(
            script,
            "namespace foo
{
    internal static class bar
    {
        /// <returns>void</returns>
        [DllImport(\"foo\", CallingConvention = CallingConvention.Cdecl, EntryPoint=\"foo\")]
        internal static extern void Foo();
    }
}\n"
        )
    }

    #[test]
    fn build_with_longer_named_void_function() {
        let mut builder =
            CSharpBuilder::new(r#"pub extern "C" fn foo_bar_zet(foo_bar: u8){}"#, "foo").unwrap();
        builder.set_namespace("foo");
        builder.set_type("bar");
        let script = builder.build();
        assert_eq!(
            script,
            "namespace foo
{
    internal static class bar
    {
        /// <param name=\"fooBar\">u8</param>
        /// <returns>void</returns>
        [DllImport(\"foo\", CallingConvention = CallingConvention.Cdecl, EntryPoint=\"foo_bar_zet\")]
        internal static extern void FooBarZet(byte fooBar);
    }
}\n"
        )
    }

    #[test]
    fn build_with_u8_function() {
        let mut builder =
            CSharpBuilder::new(r#"pub extern "C" fn foo() -> u8 { 0 }"#, "foo").unwrap();
        builder.set_namespace("foo");
        builder.set_type("bar");
        let script = builder.build();
        assert_eq!(
            script,
            "namespace foo
{
    internal static class bar
    {
        /// <returns>u8</returns>
        [DllImport(\"foo\", CallingConvention = CallingConvention.Cdecl, EntryPoint=\"foo\")]
        internal static extern byte Foo();
    }
}\n"
        )
    }

    #[test]
    fn build_with_u8_ptr_function() {
        let mut builder =
            CSharpBuilder::new(r#"pub extern "C" fn foo() -> *const u8 { 0 }"#, "foo").unwrap();
        builder.set_namespace("foo");
        builder.set_type("bar");
        let script = builder.build();
        assert_eq!(
            script,
            "namespace foo
{
    internal static class bar
    {
        /// <returns>u8*</returns>
        [DllImport(\"foo\", CallingConvention = CallingConvention.Cdecl, EntryPoint=\"foo\")]
        internal static extern IntPtr Foo();
    }
}\n"
        )
    }

    #[test]
    fn build_with_void_function_with_parameters() {
        let mut builder =
            CSharpBuilder::new(r#"pub extern "C" fn foo(a: u8, b: u8) { }"#, "foo").unwrap();
        builder.set_namespace("foo");
        builder.set_type("bar");
        let script = builder.build();
        assert_eq!(
            script,
            "namespace foo
{
    internal static class bar
    {
        /// <param name=\"a\">u8</param>
        /// <param name=\"b\">u8</param>
        /// <returns>void</returns>
        [DllImport(\"foo\", CallingConvention = CallingConvention.Cdecl, EntryPoint=\"foo\")]
        internal static extern void Foo(byte a, byte b);
    }
}\n"
        )
    }

    #[test]
    fn build_with_void_function_with_pointer_parameters() {
        let mut builder = CSharpBuilder::new(
            r#"pub extern "C" fn foo(a: *const u8, b: *const u8) {  }"#,
            "foo",
        )
        .unwrap();
        builder.set_namespace("foo");
        builder.set_type("bar");
        let script = builder.build();
        assert_eq!(
            script,
            "namespace foo
{
    internal static class bar
    {
        /// <param name=\"a\">u8*</param>
        /// <param name=\"b\">u8*</param>
        /// <returns>void</returns>
        [DllImport(\"foo\", CallingConvention = CallingConvention.Cdecl, EntryPoint=\"foo\")]
        internal static extern void Foo(IntPtr a, IntPtr b);
    }
}\n"
        )
    }

    #[test]
    fn build_with_void_function_with_outer_doc_documentation() {
        let mut builder = CSharpBuilder::new(
            r#"
            /// test documentation
            pub extern "C" fn foo(a: *const u8, b: *const u8) {  }"#,
            "foo",
        )
        .unwrap();
        builder.set_namespace("foo");
        builder.set_type("bar");
        let script = builder.build();
        assert_eq!(
            script,
            "namespace foo
{
    internal static class bar
    {
        /// <summary>
        /// test documentation
        /// </summary>
        /// <param name=\"a\">u8*</param>
        /// <param name=\"b\">u8*</param>
        /// <returns>void</returns>
        [DllImport(\"foo\", CallingConvention = CallingConvention.Cdecl, EntryPoint=\"foo\")]
        internal static extern void Foo(IntPtr a, IntPtr b);
    }
}\n"
        )
    }

    #[test]
    fn build_void_function_inside_module() {
        let mut builder =
            CSharpBuilder::new(r#"mod foo_module { pub extern "C" fn foo(){} }"#, "foo").unwrap();
        builder.set_namespace("foo");
        builder.set_type("bar");
        let script = builder.build();
        assert_eq!(
            script,
            "namespace foo
{
    internal static class bar
    {
        /// <returns>void</returns>
        [DllImport(\"foo\", CallingConvention = CallingConvention.Cdecl, EntryPoint=\"foo\")]
        internal static extern void Foo();
    }
}\n"
        )
    }

    #[test]
    fn build_enum() {
        let mut builder =
            CSharpBuilder::new(r#"#[repr(u8)] enum Foo { One, Two, Three}"#, "foo").unwrap();
        builder.set_namespace("foo");
        builder.set_type("bar");
        let script = builder.build();
        assert_eq!(
            script,
            "namespace foo
{
    internal static class bar
    {
        public enum Foo : byte
        {
            One,
            Two,
            Three,
        }
    }
}\n"
        )
    }

    #[test]
    fn build_enum_with_values() {
        let mut builder = CSharpBuilder::new(
            r#"#[repr(u8)] enum Foo { One = 1, Two = 2, Five = 5}"#,
            "foo",
        )
        .unwrap();
        builder.set_namespace("foo");
        builder.set_type("bar");
        let script = builder.build();
        assert_eq!(
            script,
            "namespace foo
{
    internal static class bar
    {
        public enum Foo : byte
        {
            One = 1,
            Two = 2,
            Five = 5,
        }
    }
}\n"
        )
    }

    #[test]
    fn build_enum_with_values_and_documentation() {
        let mut builder = CSharpBuilder::new(
            r#"#[repr(u8)] 
            /// testing documentation for enum
            enum Foo { One = 1, Two = 2, Five = 5}"#,
            "foo",
        )
        .unwrap();
        builder.set_namespace("foo");
        builder.set_type("bar");
        let script = builder.build();
        assert_eq!(
            script,
            "namespace foo
{
    internal static class bar
    {
        /// <summary>
        /// testing documentation for enum
        /// </summary>
        public enum Foo : byte
        {
            One = 1,
            Two = 2,
            Five = 5,
        }
    }
}\n"
        )
    }

    #[test]
    fn build_enum_with_values_and_documentation_for_keys() {
        let mut builder = CSharpBuilder::new(
            r#"#[repr(u8)] 
            /// testing documentation for enum
            enum Foo {
                /// Enum value one 
                One = 1, 
                /// Enum two
                Two = 2, 
                /// This is a big step!
                Five = 5
            }"#,
            "foo",
        )
        .unwrap();
        builder.set_namespace("foo");
        builder.set_type("bar");
        let script = builder.build();
        assert_eq!(
            script,
            "namespace foo
{
    internal static class bar
    {
        /// <summary>
        /// testing documentation for enum
        /// </summary>
        public enum Foo : byte
        {
            /// <summary>
            /// Enum value one
            /// </summary>
            One = 1,
            /// <summary>
            /// Enum two
            /// </summary>
            Two = 2,
            /// <summary>
            /// This is a big step!
            /// </summary>
            Five = 5,
        }
    }
}\n"
        )
    }

    #[test]
    fn build_struct() {
        let mut builder = CSharpBuilder::new(
            r#"#[repr(C)] 
            struct Foo {
                field_a: u8,
                field_b: u8,
            }"#,
            "foo",
        )
        .unwrap();
        builder.set_namespace("foo");
        builder.set_type("bar");
        let script = builder.build();
        assert_eq!(
            script,
            "namespace foo
{
    internal static class bar
    {
        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Ansi)]
        public struct Foo
        {
            /// <remarks>u8</remarks>
            public readonly byte FieldA;
            /// <remarks>u8</remarks>
            public readonly byte FieldB;
        }
    }
}\n"
        )
    }

    #[test]
    fn build_struct_with_documentation() {
        let mut builder = CSharpBuilder::new(
            r#"#[repr(C)] 
            /// test documentation struct
            struct Foo {
                /// a field. Very important!
                field_a: u8,
                /// b field. reserved or something
                field_b: u8,
            }"#,
            "foo",
        )
        .unwrap();
        builder.set_namespace("foo");
        builder.set_type("bar");
        let script = builder.build();
        assert_eq!(
            script,
            "namespace foo
{
    internal static class bar
    {
        /// <summary>
        /// test documentation struct
        /// </summary>
        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Ansi)]
        public struct Foo
        {
            /// <summary>
            /// a field. Very important!
            /// </summary>
            /// <remarks>u8</remarks>
            public readonly byte FieldA;
            /// <summary>
            /// b field. reserved or something
            /// </summary>
            /// <remarks>u8</remarks>
            public readonly byte FieldB;
        }
    }
}\n"
        )
    }
}
