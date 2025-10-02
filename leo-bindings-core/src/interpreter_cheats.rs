use crate::signature::SimplifiedBindings;
use proc_macro2::TokenStream;
use quote::quote;

pub fn generate_interpreter_cheats_from_simplified(simplified: &SimplifiedBindings) -> TokenStream {
    let program_name = &simplified.program_name;
    let cheats_module_name = syn::Ident::new(
        &format!("{}_interpreter_cheats", program_name),
        proc_macro2::Span::call_site(),
    );
    let mapping_setters = simplified.mappings.iter().map(|mapping| {
        let mapping_name = &mapping.name;
        let setter_name = syn::Ident::new(
            &format!("set_{}", mapping_name),
            proc_macro2::Span::call_site(),
        );
        let key_type = crate::types::get_rust_type(&mapping.key_type);
        let value_type = crate::types::get_rust_type(&mapping.value_type);

        quote! {
            pub fn #setter_name(key: #key_type, value: #value_type) -> Result<()> {
                with_shared_interpreter(|state| {
                    let key_value = snarkvm_value_to_leo_value(&key.to_value())?;
                    let value_value = snarkvm_value_to_leo_value(&value.to_value())?;
                    let mut interpreter = state.interpreter.borrow_mut();
                    let mapping_id = GlobalId {
                        program: Symbol::intern(#program_name),
                        path: vec![Symbol::intern(#mapping_name)],
                    };
                    interpreter.cursor.mappings.get_mut(&mapping_id)
                        .ok_or_else(|| anyhow!("Mapping '{}' not found", #mapping_name))?
                        .insert(key_value, value_value);
                    Ok(())
                })
                .ok_or_else(|| anyhow!("Shared interpreter not initialized"))?
            }
        }
    });

    quote! {
        pub mod #cheats_module_name {
            use super::*;
            use leo_bindings::{anyhow, snarkvm, leo_ast, leo_span, shared_interpreter::with_shared_interpreter, ToValue};
            use anyhow::{anyhow, Result};
            use leo_ast::interpreter_value::{GlobalId, Value};
            use leo_span::Symbol;
            use snarkvm::prelude::{TestnetV0 as Nw, Address};


            #(#mapping_setters)*
        }
    }
}
