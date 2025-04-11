extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{DeriveInput, parse_macro_input};

/// This derive macro generates asynchronous helper methods for web elements.
///
/// For every field in the struct, it always creates a base query method named
/// `query_<field>` that returns an `Option<thirtyfour::WebElement>`.
///
/// Additionally, you can annotate a field with the attribute
/// `#[thirtyfour_actions(methods(click, enter_keys))]` (or even just one method)
/// to generate extra methods for that field. Valid extra methods include:
///
/// - `click`: Generates a method `click_<field>(&self, driver: &WebDriver)` that
///   calls the base query method and then clicks the element.
/// - `enter_keys`: Generates a method `enter_keys_<field>(&self, driver: &WebDriver, keys: &str)`
///   that calls the base query and sends keys to the element.
///
/// For example:
///
/// ```rust
/// #[derive(ImplThirtyfourActions)]
/// pub struct XeroSingleFieldLoginPage {
///     #[thirtyfour_actions(methods(click, enter_keys))]
///     pub email_input: thirtyfour::By,
///     #[thirtyfour_actions(methods(click))]
///     pub login_button: thirtyfour::By,
/// }
/// ```
///
/// If a field does not have the attribute, only the base `query_<field>` is generated.
#[proc_macro_derive(ImplThirtyfourActions, attributes(thirtyfour_actions))]
pub fn impl_thirtyfour_actions(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree.
    let input_parsed = parse_macro_input!(input as DeriveInput);
    let input_span = input_parsed.span();
    let struct_name = input_parsed.ident;

    let mut methods = Vec::new();

    // Process only structs.
    if let syn::Data::Struct(data_struct) = input_parsed.data {
        for field in data_struct.fields {
            if let Some(ref field_ident) = field.ident {
                let field_name_str = field_ident.to_string();
                // Create the base query method name: query_<field>
                let query_fn_ident =
                    syn::Ident::new(&format!("query_{}", field_ident), field_ident.span());

                // Generate the base query method.
                let query_method = quote! {
                    pub async fn #query_fn_ident(&self, driver: &thirtyfour::WebDriver) -> Option<thirtyfour::WebElement> {
                        match driver.query(self.#field_ident.clone()).first_opt().await {
                            Ok(Some(element)) => Some(element),
                            Ok(None) => None,
                            Err(e) => {
                                log::error!("Error querying {}: {}", #field_name_str, e);
                                None
                            }
                        }
                    }
                };
                methods.push(query_method);

                // Look for a #[thirtyfour_actions(...)] attribute to determine extra methods.
                let mut extra_methods = Vec::new();
                for attr in &field.attrs {
                    if attr.path.is_ident("thirtyfour_actions") {
                        if let Ok(meta) = attr.parse_meta() {
                            if let syn::Meta::List(meta_list) = meta {
                                for nested in meta_list.nested.iter() {
                                    match nested {
                                        // Handle a nested list like methods(click, enter_keys)
                                        syn::NestedMeta::Meta(syn::Meta::List(inner_list)) => {
                                            if inner_list.path.is_ident("methods") {
                                                for inner in inner_list.nested.iter() {
                                                    if let syn::NestedMeta::Meta(syn::Meta::Path(
                                                        path,
                                                    )) = inner
                                                    {
                                                        if let Some(ident) = path.get_ident() {
                                                            extra_methods.push(ident.to_string());
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        // Handle key-value like methods = "click, enter_keys"
                                        syn::NestedMeta::Meta(syn::Meta::NameValue(nv)) => {
                                            if nv.path.is_ident("methods") {
                                                if let syn::Lit::Str(litstr) = &nv.lit {
                                                    let methods_str = litstr.value();
                                                    for method in methods_str.split(',') {
                                                        let method_trimmed = method.trim();
                                                        if !method_trimmed.is_empty() {
                                                            extra_methods
                                                                .push(method_trimmed.to_string());
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }

                // Now, for each extra method requested via the attribute, generate its implementation.
                for extra in extra_methods {
                    match extra.as_str() {
                        "click" => {
                            let click_fn_ident = syn::Ident::new(
                                &format!("click_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                pub async fn #click_fn_ident(&self, driver: &thirtyfour::WebDriver) -> anyhow::Result<()> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            element.click().await
                                                .context(concat!("Failed to click ", #field_name_str))?;
                                            Ok(())
                                        },
                                        None => Err(anyhow::anyhow!(concat!(#field_name_str, " not found")))
                                    }
                                }
                            };
                            methods.push(method);
                        }
                        "enter_keys" => {
                            let enter_fn_ident = syn::Ident::new(
                                &format!("enter_keys_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                pub async fn #enter_fn_ident(&self, driver: &thirtyfour::WebDriver, keys: &str) -> anyhow::Result<()> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(input) => {
                                            input.send_keys(keys).await
                                                .context(concat!("Failed to send keys to ", #field_name_str))?;
                                            Ok(())
                                        },
                                        None => Err(anyhow::anyhow!(concat!(#field_name_str, " not found")))
                                    }
                                }
                            };
                            methods.push(method);
                        }
                        _ => {} // Unsupported extra method names are ignored.
                    }
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

    let expanded = quote! {
        impl #struct_name {
            #(#methods)*
        }
    };

    TokenStream::from(expanded)
}
