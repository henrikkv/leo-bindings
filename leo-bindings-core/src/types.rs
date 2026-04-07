use convert_case::{Case, Casing};
use indexmap::IndexMap;
use leo_abi_types as abi;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use snarkvm::prelude::*;
use std::sync::OnceLock;

pub fn get_rust_type(ty: &abi::Plaintext) -> TokenStream {
    match ty {
        abi::Plaintext::Primitive(p) => match p {
            abi::Primitive::Address => quote! { Address<N> },
            abi::Primitive::Boolean => quote! { bool },
            abi::Primitive::Field => quote! { Field<N> },
            abi::Primitive::Group => quote! { Group<N> },
            abi::Primitive::Scalar => quote! { Scalar<N> },
            abi::Primitive::Signature => quote! { Signature<N> },
            abi::Primitive::Identifier => quote! { IdentifierLiteral<N> },
            abi::Primitive::Int(i) => match i {
                abi::Int::I8 => quote! { i8 },
                abi::Int::I16 => quote! { i16 },
                abi::Int::I32 => quote! { i32 },
                abi::Int::I64 => quote! { i64 },
                abi::Int::I128 => quote! { i128 },
            },
            abi::Primitive::UInt(u) => match u {
                abi::UInt::U8 => quote! { u8 },
                abi::UInt::U16 => quote! { u16 },
                abi::UInt::U32 => quote! { u32 },
                abi::UInt::U64 => quote! { u64 },
                abi::UInt::U128 => quote! { u128 },
            },
        },
        abi::Plaintext::Array(array) => {
            let element_type = get_rust_type(array.element.as_ref());
            let size: usize = array.length as usize;
            quote! { [#element_type; #size] }
        }
        abi::Plaintext::Struct(sref) => {
            let last = sref
                .path
                .last()
                .expect("StructRef.path should have at least one segment");
            let struct_ident = syn::Ident::new(&last.to_case(Case::Pascal), Span::call_site());
            quote! { #struct_ident<N> }
        }
        abi::Plaintext::Optional(opt) => {
            let inner_type = get_rust_type(opt.0.as_ref());
            quote! { Option<#inner_type> }
        }
    }
}

/// Rust type for a record reference in the ABI
pub fn get_rust_type_from_record_ref(r: &abi::RecordRef) -> TokenStream {
    let last = r
        .path
        .last()
        .expect("RecordRef.path should have at least one segment");
    let record_ident = syn::Ident::new(&last.to_case(Case::Pascal), Span::call_site());
    quote! { #record_ident<N> }
}

/// Rust parameter type for a function input from `abi.json`.
pub fn get_rust_type_from_function_input(ty: &abi::FunctionInput) -> TokenStream {
    match ty {
        abi::FunctionInput::Plaintext(p) => get_rust_type(p),
        abi::FunctionInput::Record(r) => get_rust_type_from_record_ref(r),
        abi::FunctionInput::DynamicRecord => {
            quote! { Record<N, Plaintext<N>> }
        }
    }
}

/// Rust return type for a function output from `abi.json`.
pub fn get_rust_type_from_function_output(ty: &abi::FunctionOutput) -> TokenStream {
    match ty {
        abi::FunctionOutput::Plaintext(p) => get_rust_type(p),
        abi::FunctionOutput::Record(r) => get_rust_type_from_record_ref(r),
        abi::FunctionOutput::Final => quote! { Future<N> },
        abi::FunctionOutput::DynamicRecord => {
            quote! { Record<N, Plaintext<N>> }
        }
    }
}

pub struct ArrayInfo {
    pub element_type: String,
    pub size: usize,
}

pub fn parse_array_type(type_name: &str) -> Option<ArrayInfo> {
    let trimmed = type_name.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return None;
    }

    let inner = &trimmed[1..trimmed.len() - 1];

    if let Some(semicolon_pos) = inner.rfind(';') {
        let element_type = inner[..semicolon_pos].trim().to_string();
        let size_str = inner[semicolon_pos + 1..].trim();
        if let Ok(size) = size_str.parse::<usize>() {
            return Some(ArrayInfo { element_type, size });
        }
    }

    None
}

/// Converts Rust types to SnarkVM types.
pub trait ToValue<N: Network> {
    fn to_value(&self) -> Value<N>;
}

/// Converts SnarkVM types to Rust types.
pub trait FromValue<N: Network> {
    fn from_value(value: Value<N>) -> Self;
}

impl<N: Network, T> ToValue<N> for Option<T>
where
    T: ToValue<N> + Default,
{
    fn to_value(&self) -> Value<N> {
        let is_some_bool = self.is_some();

        let is_some_plaintext = match ToValue::<N>::to_value(&is_some_bool) {
            Value::Plaintext(p) => p,
            _ => panic!("Expected plaintext boolean for lowered optional `is_some`."),
        };

        let val_plaintext = match self {
            Some(v) => match ToValue::<N>::to_value(v) {
                Value::Plaintext(p) => p,
                _ => panic!("Expected plaintext value for lowered optional `val`."),
            },
            None => match ToValue::<N>::to_value(&T::default()) {
                Value::Plaintext(p) => p,
                _ => panic!("Expected plaintext value for lowered optional `val` (default)."),
            },
        };

        let members = IndexMap::from([
            (Identifier::try_from("is_some").unwrap(), is_some_plaintext),
            (Identifier::try_from("val").unwrap(), val_plaintext),
        ]);

        Value::Plaintext(Plaintext::Struct(members, OnceLock::new()))
    }
}

impl<N: Network, T> FromValue<N> for Option<T>
where
    T: FromValue<N>,
{
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(Plaintext::Struct(struct_members, _)) => {
                let is_some_id = Identifier::try_from("is_some").unwrap();
                let val_id = Identifier::try_from("val").unwrap();

                let is_some_plaintext = struct_members
                    .get(&is_some_id)
                    .expect("Lowered optional missing `is_some` field");

                let is_some = bool::from_value(Value::Plaintext(is_some_plaintext.clone()));

                if !is_some {
                    return None;
                }

                let val_plaintext = struct_members
                    .get(&val_id)
                    .expect("Lowered optional missing `val` field");

                Some(T::from_value(Value::Plaintext(val_plaintext.clone())))
            }
            _ => panic!("Expected lowered optional as a plaintext struct"),
        }
    }
}

impl<N: Network> ToValue<N> for u8 {
    fn to_value(&self) -> Value<N> {
        Value::Plaintext(Plaintext::from(Literal::U8(U8::new(*self))))
    }
}

impl<N: Network> FromValue<N> for u8 {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(plaintext) => match plaintext {
                Plaintext::Literal(literal, _) => match literal {
                    Literal::U8(u8_val) => *u8_val,
                    _ => panic!("Expected u8 type"),
                },
                _ => panic!("Expected literal plaintext"),
            },
            _ => panic!("Expected plaintext value"),
        }
    }
}

impl<N: Network> ToValue<N> for u16 {
    fn to_value(&self) -> Value<N> {
        Value::Plaintext(Plaintext::from(Literal::U16(U16::new(*self))))
    }
}

impl<N: Network> FromValue<N> for u16 {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(plaintext) => match plaintext {
                Plaintext::Literal(literal, _) => match literal {
                    Literal::U16(u16_val) => *u16_val,
                    _ => panic!("Expected u16 type"),
                },
                _ => panic!("Expected literal plaintext"),
            },
            _ => panic!("Expected plaintext value"),
        }
    }
}

impl<N: Network> ToValue<N> for u32 {
    fn to_value(&self) -> Value<N> {
        Value::Plaintext(Plaintext::from(Literal::U32(U32::new(*self))))
    }
}

impl<N: Network> FromValue<N> for u32 {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(plaintext) => match plaintext {
                Plaintext::Literal(literal, _) => match literal {
                    Literal::U32(u32_val) => *u32_val,
                    _ => panic!("Expected u32 type"),
                },
                _ => panic!("Expected literal plaintext"),
            },
            _ => panic!("Expected plaintext value"),
        }
    }
}

impl<N: Network> ToValue<N> for u64 {
    fn to_value(&self) -> Value<N> {
        Value::Plaintext(Plaintext::from(Literal::U64(U64::new(*self))))
    }
}

impl<N: Network> FromValue<N> for u64 {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(plaintext) => match plaintext {
                Plaintext::Literal(literal, _) => match literal {
                    Literal::U64(u64_val) => *u64_val,
                    _ => panic!("Expected u64 type"),
                },
                _ => panic!("Expected literal plaintext"),
            },
            _ => panic!("Expected plaintext value"),
        }
    }
}

impl<N: Network> ToValue<N> for u128 {
    fn to_value(&self) -> Value<N> {
        Value::Plaintext(Plaintext::from(Literal::U128(U128::new(*self))))
    }
}

impl<N: Network> FromValue<N> for u128 {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(plaintext) => match plaintext {
                Plaintext::Literal(literal, _) => match literal {
                    Literal::U128(u128_val) => *u128_val,
                    _ => panic!("Expected u128 type"),
                },
                _ => panic!("Expected literal plaintext"),
            },
            _ => panic!("Expected plaintext value"),
        }
    }
}

impl<N: Network> ToValue<N> for bool {
    fn to_value(&self) -> Value<N> {
        Value::Plaintext(Plaintext::from(Literal::Boolean(Boolean::new(*self))))
    }
}

impl<N: Network> FromValue<N> for bool {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(plaintext) => match plaintext {
                Plaintext::Literal(literal, _) => match literal {
                    Literal::Boolean(bool_val) => *bool_val,
                    _ => panic!("Expected bool type"),
                },
                _ => panic!("Expected literal plaintext"),
            },
            _ => panic!("Expected plaintext value"),
        }
    }
}

impl<N: Network> ToValue<N> for Address<N> {
    fn to_value(&self) -> Value<N> {
        Value::from(Literal::Address(*self))
    }
}

impl<N: Network> FromValue<N> for Address<N> {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(Plaintext::Literal(Literal::Address(v), _)) => v,
            _ => panic!("Expected address type."),
        }
    }
}

impl<N: Network> ToValue<N> for Field<N> {
    fn to_value(&self) -> Value<N> {
        Value::Plaintext(Plaintext::from(Literal::Field(*self)))
    }
}

impl<N: Network> FromValue<N> for Field<N> {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(plaintext) => match plaintext {
                Plaintext::Literal(literal, _) => match literal {
                    Literal::Field(field_val) => field_val,
                    _ => panic!("Expected field type"),
                },
                _ => panic!("Expected literal plaintext"),
            },
            _ => panic!("Expected plaintext value"),
        }
    }
}

impl<N: Network> ToValue<N> for i8 {
    fn to_value(&self) -> Value<N> {
        Value::Plaintext(Plaintext::from(Literal::I8(I8::new(*self))))
    }
}

impl<N: Network> FromValue<N> for i8 {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(plaintext) => match plaintext {
                Plaintext::Literal(literal, _) => match literal {
                    Literal::I8(i8_val) => *i8_val,
                    _ => panic!("Expected i8 type"),
                },
                _ => panic!("Expected literal plaintext"),
            },
            _ => panic!("Expected plaintext value"),
        }
    }
}

impl<N: Network> ToValue<N> for i16 {
    fn to_value(&self) -> Value<N> {
        Value::Plaintext(Plaintext::from(Literal::I16(I16::new(*self))))
    }
}

impl<N: Network> FromValue<N> for i16 {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(plaintext) => match plaintext {
                Plaintext::Literal(literal, _) => match literal {
                    Literal::I16(i16_val) => *i16_val,
                    _ => panic!("Expected i16 type"),
                },
                _ => panic!("Expected literal plaintext"),
            },
            _ => panic!("Expected plaintext value"),
        }
    }
}

impl<N: Network> ToValue<N> for i32 {
    fn to_value(&self) -> Value<N> {
        Value::Plaintext(Plaintext::from(Literal::I32(I32::new(*self))))
    }
}

impl<N: Network> FromValue<N> for i32 {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(plaintext) => match plaintext {
                Plaintext::Literal(literal, _) => match literal {
                    Literal::I32(i32_val) => *i32_val,
                    _ => panic!("Expected i32 type"),
                },
                _ => panic!("Expected literal plaintext"),
            },
            _ => panic!("Expected plaintext value"),
        }
    }
}

impl<N: Network> ToValue<N> for i64 {
    fn to_value(&self) -> Value<N> {
        Value::Plaintext(Plaintext::from(Literal::I64(I64::new(*self))))
    }
}

impl<N: Network> FromValue<N> for i64 {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(plaintext) => match plaintext {
                Plaintext::Literal(literal, _) => match literal {
                    Literal::I64(i64_val) => *i64_val,
                    _ => panic!("Expected i64 type"),
                },
                _ => panic!("Expected literal plaintext"),
            },
            _ => panic!("Expected plaintext value"),
        }
    }
}

impl<N: Network> ToValue<N> for i128 {
    fn to_value(&self) -> Value<N> {
        Value::Plaintext(Plaintext::from(Literal::I128(I128::new(*self))))
    }
}

impl<N: Network> FromValue<N> for i128 {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(plaintext) => match plaintext {
                Plaintext::Literal(literal, _) => match literal {
                    Literal::I128(i128_val) => *i128_val,
                    _ => panic!("Expected i128 type"),
                },
                _ => panic!("Expected literal plaintext"),
            },
            _ => panic!("Expected plaintext value"),
        }
    }
}

impl<N: Network> ToValue<N> for Group<N> {
    fn to_value(&self) -> Value<N> {
        Value::Plaintext(Plaintext::from(Literal::Group(*self)))
    }
}

impl<N: Network> FromValue<N> for Group<N> {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(plaintext) => match plaintext {
                Plaintext::Literal(literal, _) => match literal {
                    Literal::Group(group_val) => group_val,
                    _ => panic!("Expected group type"),
                },
                _ => panic!("Expected literal plaintext"),
            },
            _ => panic!("Expected plaintext value"),
        }
    }
}

impl<N: Network> ToValue<N> for Scalar<N> {
    fn to_value(&self) -> Value<N> {
        Value::Plaintext(Plaintext::from(Literal::Scalar(*self)))
    }
}

impl<N: Network> FromValue<N> for Scalar<N> {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(plaintext) => match plaintext {
                Plaintext::Literal(literal, _) => match literal {
                    Literal::Scalar(scalar_val) => scalar_val,
                    _ => panic!("Expected scalar type"),
                },
                _ => panic!("Expected literal plaintext"),
            },
            _ => panic!("Expected plaintext value"),
        }
    }
}

impl<N: Network> ToValue<N> for Signature<N> {
    fn to_value(&self) -> Value<N> {
        Value::Plaintext(Plaintext::from(Literal::Signature(Box::new(*self))))
    }
}

impl<N: Network> FromValue<N> for Signature<N> {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(plaintext) => match plaintext {
                Plaintext::Literal(literal, _) => match literal {
                    Literal::Signature(signature_val) => *signature_val,
                    _ => panic!("Expected signature type"),
                },
                _ => panic!("Expected literal plaintext"),
            },
            _ => panic!("Expected plaintext value"),
        }
    }
}

impl<N: Network> ToValue<N> for StringType<N> {
    fn to_value(&self) -> Value<N> {
        Value::Plaintext(Plaintext::from(Literal::String(self.clone())))
    }
}

impl<N: Network> FromValue<N> for StringType<N> {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(plaintext) => match plaintext {
                Plaintext::Literal(literal, _) => match literal {
                    Literal::String(string_val) => string_val,
                    _ => panic!("Expected string type"),
                },
                _ => panic!("Expected literal plaintext"),
            },
            _ => panic!("Expected plaintext value"),
        }
    }
}

impl<N: Network> ToValue<N> for IdentifierLiteral<N> {
    fn to_value(&self) -> Value<N> {
        Value::Plaintext(Plaintext::from(Literal::Identifier(Box::new(*self))))
    }
}

impl<N: Network> FromValue<N> for IdentifierLiteral<N> {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(plaintext) => match plaintext {
                Plaintext::Literal(literal, _) => match literal {
                    Literal::Identifier(identifier_val) => *identifier_val,
                    _ => panic!("Expected identifier type"),
                },
                _ => panic!("Expected literal plaintext"),
            },
            _ => panic!("Expected plaintext value"),
        }
    }
}

impl<N: Network> ToValue<N> for Ciphertext<N> {
    fn to_value(&self) -> Value<N> {
        panic!("Ciphertext can not be converted")
    }
}

impl<N: Network> FromValue<N> for Ciphertext<N> {
    fn from_value(_value: Value<N>) -> Self {
        panic!("Ciphertext can not be converted")
    }
}

impl<N: Network> ToValue<N> for Future<N> {
    fn to_value(&self) -> Value<N> {
        Value::Future(self.clone())
    }
}

impl<N: Network> FromValue<N> for Future<N> {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Future(future) => future,
            _ => panic!("Expected future value"),
        }
    }
}

impl<N: Network> ToValue<N> for Entry<N, Plaintext<N>> {
    fn to_value(&self) -> Value<N> {
        match self {
            Entry::Public(entry) | Entry::Private(entry) | Entry::Constant(entry) => {
                Value::Plaintext(entry.clone())
            }
        }
    }
}

impl<N: Network> ToValue<N> for Record<N, Plaintext<N>> {
    fn to_value(&self) -> Value<N> {
        Value::Record(self.clone())
    }
}

impl<N: Network> FromValue<N> for Record<N, Plaintext<N>> {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Record(record) => record,
            _ => panic!("Expected record value"),
        }
    }
}

impl<N: Network> ToValue<N> for Record<N, Ciphertext<N>> {
    fn to_value(&self) -> Value<N> {
        panic!("Encrypted records must be decrypted first")
    }
}

impl<N: Network> FromValue<N> for Record<N, Ciphertext<N>> {
    fn from_value(_value: Value<N>) -> Self {
        panic!("Cannot create encrypted record from Value")
    }
}

impl<N: Network, T: ToValue<N> + Copy, const SIZE: usize> ToValue<N> for [T; SIZE] {
    fn to_value(&self) -> Value<N> {
        let array_elements: Vec<Plaintext<N>> = self
            .iter()
            .map(|item| match item.to_value() {
                Value::Plaintext(p) => p,
                _ => panic!("Expected plaintext value from array element"),
            })
            .collect();

        Value::Plaintext(Plaintext::Array(array_elements, std::sync::OnceLock::new()))
    }
}

impl<N: Network, T: FromValue<N>, const SIZE: usize> FromValue<N> for [T; SIZE] {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(Plaintext::Array(array_elements, _)) => {
                if array_elements.len() != SIZE {
                    panic!(
                        "Array size mismatch: expected {}, got {}",
                        SIZE,
                        array_elements.len()
                    );
                }

                let mut iter = array_elements.into_iter();
                std::array::from_fn(|_| {
                    let element = iter.next().expect("length checked above");
                    T::from_value(Value::Plaintext(element))
                })
            }
            _ => panic!("Expected array type"),
        }
    }
}
