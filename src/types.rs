use proc_macro2::TokenStream;
use quote::quote;
use snarkvm::prelude::*;

pub fn get_rust_type(type_name: &str) -> TokenStream {
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
        "Future" => quote! { Future<Nw> },
        other => {
            let type_ident = syn::Ident::new(other, proc_macro2::Span::call_site());
            quote! { #type_ident }
        }
    }
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

