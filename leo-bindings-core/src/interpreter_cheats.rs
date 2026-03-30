use crate::get_rust_type;
use crate::signature::SimplifiedBindings;
use proc_macro2::{Span, TokenStream};
use quote::quote;

pub(crate) fn generate_interpreter_cheats_from_simplified(
    simplified: &SimplifiedBindings,
) -> TokenStream {
    let program_id = &simplified.program_id;
    let cheats_module_name = syn::Ident::new(
        &format!("{}_interpreter_cheats", program_id),
        Span::call_site(),
    );
    let mapping_setters = simplified.mappings.iter().map(|mapping| {
        let mapping_name = &mapping.name;
        let setter_name = syn::Ident::new(&format!("set_{}", mapping_name), Span::call_site());
        let key_type = get_rust_type(&mapping.key_type);
        let value_type = get_rust_type(&mapping.value_type);

        quote! {
            /// Setter for a mapping in the interpreter.
            pub fn #setter_name(key: #key_type, value: #value_type) -> Result<()> {
                let svm_key = ToValue::<TestnetV0>::to_value(&key);
                let svm_value = ToValue::<TestnetV0>::to_value(&value);
                with_interpreter_blocking(move |state| {
                    let key_value = leo_ast::interpreter_value::Value::from(svm_key);
                    let value_value = leo_ast::interpreter_value::Value::from(svm_value);
                    let mut interpreter = state.interpreter.borrow_mut();
                    let mapping_id = leo_ast::Location::new(
                        Symbol::intern(#program_id),
                        vec![Symbol::intern(#mapping_name)],
                    );
                    interpreter.cursor.mappings.get_mut(&mapping_id)
                        .ok_or_else(|| anyhow!("Mapping '{}' not found", #mapping_name)).unwrap()
                        .insert(key_value, value_value);
                })
                .ok_or_else(|| anyhow!("Shared interpreter not initialized"))
            }
        }
    });

    quote! {
        /// Cheats for testing with the interpreter bindings.
        pub mod #cheats_module_name {
            use super::*;
            use leo_bindings::{anyhow, leo_ast, leo_span, shared_interpreter::with_interpreter_blocking, ToValue};
            use anyhow::{anyhow, Result};
            use leo_span::Symbol;
            use snarkvm::prelude::TestnetV0;

            #(#mapping_setters)*
        }
    }
}
