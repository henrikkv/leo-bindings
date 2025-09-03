use proc_macro2::TokenStream;
use quote::quote;
use snarkvm::prelude::*;

pub fn get_rust_type(type_name: &str) -> TokenStream {
    if let Some(array_info) = parse_array_type(type_name) {
        let inner_type = get_rust_type(&array_info.element_type);
        let size = array_info.size;
        return quote! { [#inner_type; #size] };
    }

    match type_name {
        "u8" => quote! { u8 },
        "u16" => quote! { u16 },
        "u32" => quote! { u32 },
        "u64" => quote! { u64 },
        "u128" => quote! { u128 },
        "i8" => quote! { i8 },
        "i16" => quote! { i16 },
        "i32" => quote! { i32 },
        "i64" => quote! { i64 },
        "i128" => quote! { i128 },
        "address" => quote! { Address<Nw> },
        "Address" => quote! { Address<Nw> },
        "field" => quote! { Field<Nw> },
        "Field" => quote! { Field<Nw> },
        "group" => quote! { Group<Nw> },
        "Group" => quote! { Group<Nw> },
        "scalar" => quote! { Scalar<Nw> },
        "Scalar" => quote! { Scalar<Nw> },
        "signature" => quote! { Signature<Nw> },
        "Signature" => quote! { Signature<Nw> },
        "string" => quote! { StringType<Nw> },
        "String" => quote! { StringType<Nw> },
        "bool" => quote! { bool },
        "boolean" => quote! { bool },
        "Boolean" => quote! { bool },
        "Future" => quote! { Future<Nw> },
        "Ciphertext" => quote! { Ciphertext<Nw> },
        other => {
            let type_ident = syn::Ident::new(other, proc_macro2::Span::call_site());
            quote! { #type_ident }
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

pub trait ToValue<N: Network> {
    fn to_value(&self) -> Value<N>;
}

pub trait FromValue<N: Network> {
    fn from_value(value: Value<N>) -> Self;
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
