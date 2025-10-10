use crate::signature::SimplifiedBindings;
use proc_macro2::{Span, TokenStream};
use quote::quote;

pub fn generate_interpreter_cheats_from_simplified(simplified: &SimplifiedBindings) -> TokenStream {
    let program_name = &simplified.program_name;
    let cheats_module_name = syn::Ident::new(
        &format!("{}_interpreter_cheats", program_name),
        Span::call_site(),
    );
    let mapping_setters = simplified.mappings.iter().map(|mapping| {
        let mapping_name = &mapping.name;
        let setter_name = syn::Ident::new(&format!("set_{}", mapping_name), Span::call_site());
        let key_type = crate::types::get_rust_type_with_network(&mapping.key_type, "Nw");
        let value_type = crate::types::get_rust_type_with_network(&mapping.value_type, "Nw");

        quote! {
            pub fn #setter_name(key: #key_type, value: #value_type) -> Result<()> {
                with_shared_interpreter(|state| {
                    let key_value = leo_ast::interpreter_value::Value::from((key).to_value());
                    let value_value = leo_ast::interpreter_value::Value::from((value).to_value());
                    let mut interpreter = state.interpreter.borrow_mut();
                    let mapping_id = leo_ast::Location::new(
                        Symbol::intern(#program_name),
                        vec![Symbol::intern(#mapping_name)],
                    );
                    interpreter.cursor.mappings.get_mut(&mapping_id)
                        .ok_or_else(|| anyhow!("Mapping '{}' not found", #mapping_name)).unwrap()
                        .insert(key_value, value_value);
                    Ok(())
                })
                .ok_or_else(|| anyhow!("Shared interpreter not initialized")).unwrap()
            }
        }
    });

    quote! {
        pub mod #cheats_module_name {
            use super::*;
            use leo_bindings::{anyhow, snarkvm, leo_ast, leo_span, shared_interpreter::with_shared_interpreter, ToValue, FromValue};
            use anyhow::{anyhow, Result};
            use leo_ast::interpreter_value::Value;
            use leo_span::Symbol;
            use snarkvm::prelude::{TestnetV0 as Nw, Address};

            #(#mapping_setters)*
        }
    }
}
