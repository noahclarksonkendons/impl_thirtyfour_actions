extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{DeriveInput, parse_macro_input}; // Bring the Spanned trait into scope

/// The custom derive macro automatically generates query and click methods for
/// every field whose type includes "By".
#[proc_macro_derive(ImplThirtyfourActions)]
pub fn impl_thirtyfour_actions(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let input_parsed = parse_macro_input!(input as DeriveInput);
    let input_span = input_parsed.span(); // Capture the span *before* consuming input.ident
    let struct_name = input_parsed.ident;

    let mut methods = Vec::new();

    // Work only for structs.
    if let syn::Data::Struct(data_struct) = input_parsed.data {
        // Iterate over each field in the struct.
        for field in data_struct.fields {
            // Borrow the identifier to avoid consuming it.
            if let Some(ref field_ident) = field.ident {
                // Convert the field type to string for a simple check.
                let field_ty_str = quote!(#field.ty).to_string();
                if field_ty_str.contains("By") {
                    let field_name = field_ident.to_string();

                    // Create new identifiers for the generated methods,
                    // using the span of the field_ident.
                    let query_fn_ident = syn::Ident::new(
                        &format!("query_present_{}", field_ident),
                        field_ident.span(),
                    );
                    let click_fn_ident =
                        syn::Ident::new(&format!("click_{}", field_ident), field_ident.span());

                    methods.push(quote! {
                        /// Asynchronously queries for the presence of the element.
                        pub async fn #query_fn_ident(&self, driver: &thirtyfour::WebDriver) -> Option<thirtyfour::WebElement> {
                            match driver.query(self.#field_ident.clone()).first_opt().await {
                                Ok(Some(element)) => Some(element),
                                Ok(None) => None,
                                Err(e) => {
                                    log::error!("Error querying {}: {}", #field_name, e);
                                    None
                                }
                            }
                        }

                        /// Asynchronously attempts to click the element.
                        pub async fn #click_fn_ident(&self, driver: &thirtyfour::WebDriver) -> anyhow::Result<()> {
                            match self.#query_fn_ident(driver).await {
                                Some(element) => {
                                    element.click().await
                                        .context(concat!("Failed to click ", #field_name))?;
                                    Ok(())
                                },
                                None => Err(anyhow::anyhow!(concat!(#field_name, " not found")))
                            }
                        }
                    });
                }
            }
        }
    } else {
        return syn::Error::new(
            input_span,
            "ImplThirtyfourActions can only be derived for structs",
        )
        .to_compile_error()
        .into();
    }

    // Generate the implementation block for the struct with all generated methods.
    let expanded = quote! {
        impl #struct_name {
            #(#methods)*
        }
    };

    TokenStream::from(expanded)
}
