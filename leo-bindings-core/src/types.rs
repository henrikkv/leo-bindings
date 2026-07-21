use convert_case::{Case, Casing};
use leo_abi_types as abi;
use proc_macro2::{Span, TokenStream};
use quote::quote;

fn pascal_ident_from_path(path: &abi::Path) -> syn::Ident {
    let last = path
        .last()
        .expect("Type path should have at least one segment");
    syn::Ident::new(&last.to_case(Case::Pascal), Span::call_site())
}

fn record_rust_type(path: &abi::Path) -> TokenStream {
    let id = pascal_ident_from_path(path);
    quote! { #id<N> }
}

pub trait ToRustType {
    fn to_rust_type(&self) -> TokenStream;
}

impl ToRustType for abi::Plaintext {
    fn to_rust_type(&self) -> TokenStream {
        rust_type_plaintext(self)
    }
}

impl ToRustType for abi::FunctionInput {
    fn to_rust_type(&self) -> TokenStream {
        match self {
            abi::FunctionInput::Plaintext { ty, .. } => rust_type_plaintext(ty),
            abi::FunctionInput::Record(rec) => record_rust_type(&rec.path),
            abi::FunctionInput::DynamicRecord => quote! { DynamicRecord<N> },
        }
    }
}

impl ToRustType for abi::FunctionOutput {
    fn to_rust_type(&self) -> TokenStream {
        match self {
            abi::FunctionOutput::Plaintext { ty, .. } => rust_type_plaintext(ty),
            abi::FunctionOutput::Record(rec) => record_rust_type(&rec.path),
            abi::FunctionOutput::Final => quote! { Future<N> },
            abi::FunctionOutput::DynamicRecord => quote! { DynamicRecord<N> },
        }
    }
}

fn rust_type_plaintext(p: &abi::Plaintext) -> TokenStream {
    match p {
        abi::Plaintext::Primitive(prim) => match prim {
            abi::Primitive::Address => quote! { Address<N> },
            abi::Primitive::Boolean => quote! { bool },
            abi::Primitive::Field => quote! { Field<N> },
            abi::Primitive::Group => quote! { Group<N> },
            abi::Primitive::Scalar => quote! { Scalar<N> },
            abi::Primitive::Signature => quote! { Signature<N> },
            abi::Primitive::Identifier => quote! { Identifier<N> },
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
            let element_type = rust_type_plaintext(array.element.as_ref());
            let size: usize = array.length as usize;
            quote! { [#element_type; #size] }
        }
        abi::Plaintext::Struct(sref) => record_rust_type(&sref.path),
        abi::Plaintext::Optional(opt) => {
            let inner_type = rust_type_plaintext(opt.0.as_ref());
            quote! { Option<#inner_type> }
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
