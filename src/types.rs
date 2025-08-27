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
        "bool" => quote! { bool },
        "boolean" => quote! { bool },
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
