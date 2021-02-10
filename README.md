[![crates.io](https://img.shields.io/crates/v/csharp_binder.svg)](https://crates.io/crates/csharp_binder)

## CSharp_Binder

CSharp_Binder is a tool written to generate C# bindings for a Rust FFI (Foreign Function Interface).
By interacting over extern C functions, this allows you to easily call Rust functions from C#,
without having to write the extern C# functions yourself.

CSharp_Binder will when given a Rust script, parse this script, and extract any functions marked as
extern "C", enums with a ``[repr(u*)]`` attribute, and structs with a ``#[repr(C)]`` attribute. It
will then convert these into appropriate representations in C#.

CSharp_Binder will also extract Rust documentation on functions, enums and their variants, and
on structs and their fields, and convert it into XML Documentation on the generated C# code.

Note that CSharp_Binder uses syn to parse Rust scripts, so macros will not be expanded! If you
have functions, structs, or enums that need to be extracted inside macros, make sure to run them
to something like cargo-expand first.

## Examples

Example:
```rust
use csharp_binder::{CSharpConfiguration, CSharpBuilder};

fn main(){
    // Create C# configuration with C# target version 9.
    let mut configuration = CSharpConfiguration::new(9);
    let rust_file = r#"
    /// Just a random return enum
    #[repr(u8)]
    enum ReturnEnum {
        Val1,
        Val2,
    }

    /// An input struct we expect
    #[repr(C)]
    struct InputStruct {
        field_a: u16,
        /// This field is used for floats!
        field_b: f64,
    }

    pub extern "C" fn foo(a: InputStruct) -> ReturnEnum {
    }
    "#;
    let mut builder = CSharpBuilder::new(rust_file, "foo", &mut configuration)
                        .expect("Failed to parse file");
    builder.set_namespace("MainNamespace");
    builder.set_type("InsideClass");
    let script = builder.build().expect("Failed to build");
}
```

This would return the following C# code:

```cs
// Automatically generated, do not edit!
using System;
using System.Runtime.InteropServices;

namespace MainNamespace
{
   internal static class InsideClass
   {
       /// <summary>
       /// Just a random return enum
       /// </summary>
        public enum ReturnEnum : byte
        {
            Val1,
            Val2,
        }

        /// <summary>
        /// An input struct we expect
        /// </summary>
        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        public struct InputStruct
        {
            /// <remarks>u16</remarks>
            public ushort FieldA { get; init; }
            /// <summary>
            /// This field is used for floats!
            /// </summary>
            /// <remarks>f64</remarks>
            public double FieldB { get; init; }

            public InputStruct(ushort fieldA, double fieldB)
            {
                FieldA = fieldA;
                FieldB = fieldB;
            }
        }

        /// <param name="a">InputStruct</param>
        /// <returns>ReturnEnum</returns>
        [DllImport("foo", CallingConvention = CallingConvention.Cdecl, EntryPoint="foo")]
        internal static extern ReturnEnum Foo(InputStruct a);

    }
}
```

