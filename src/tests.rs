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
        let script = builder.build().unwrap();
        assert_eq!(script, "namespace foo\n{\n}\n")
    }

    #[test]
    fn build_empty_with_type() {
        let mut builder = CSharpBuilder::new(r#""#, "foo").unwrap();
        builder.set_type("foo");
        let script = builder.build().unwrap();
        assert_eq!(script, "internal static class foo\n{\n}\n")
    }

    #[test]
    fn build_empty_with_namespace_and_type() {
        let mut builder = CSharpBuilder::new(r#""#, "foo").unwrap();
        builder.set_namespace("foo");
        builder.set_type("bar");
        let script = builder.build().unwrap();
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
        let script = builder.build().unwrap();
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
        let script = builder.build().unwrap();
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
        let script = builder.build().unwrap();
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
        let script = builder.build().unwrap();
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
        let script = builder.build().unwrap();
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
        let script = builder.build().unwrap();
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
        let script = builder.build().unwrap();
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
        let script = builder.build().unwrap();
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
        let script = builder.build().unwrap();
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
        let script = builder.build().unwrap();
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
        let script = builder.build().unwrap();
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
        let script = builder.build().unwrap();
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
        let script = builder.build().unwrap();
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
        let script = builder.build().unwrap();
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
