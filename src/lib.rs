extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::Ident;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{DeriveInput, LitStr, parse_macro_input, spanned::Spanned};

struct ElementMethods {
    methods: Vec<String>,
}

impl Parse for ElementMethods {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Expect the keyword "methods"
        let ident: Ident = input.parse()?;
        if ident != "methods" {
            return Err(syn::Error::new(ident.span(), "expected 'methods'"));
        }

        // Parse the parenthesized content
        let content;
        syn::parenthesized!(content in input);

        // Parse comma-separated identifiers
        let method_names = Punctuated::<Ident, Comma>::parse_terminated(&content)?;
        let methods = method_names.into_iter().map(|id| id.to_string()).collect();

        Ok(ElementMethods { methods })
    }
}

/// The custom derive macro automatically generates asynchronous helper methods for web elements.
///
/// For every field in the struct, it always generates a base query method named:
///     query_<field>(&self, driver: &thirtyfour::WebDriver)
///
/// If a field is annotated with the attribute:
///     #[thirtyfour_actions(methods(click, enter_keys, get_text, is_displayed))]
///
/// then additional methods are generated for each requested action.
#[proc_macro_derive(ImplThirtyfourActions, attributes(thirtyfour_actions))]
pub fn impl_thirtyfour_actions(input: TokenStream) -> TokenStream {
    let input_parsed = parse_macro_input!(input as DeriveInput);
    let input_span = input_parsed.span();
    let struct_name = input_parsed.ident;

    let mut methods = Vec::new();

    if let syn::Data::Struct(data_struct) = input_parsed.data {
        for field in data_struct.fields {
            if let Some(ref field_ident) = field.ident {
                let field_name_str = field_ident.to_string();
                // Always generate the base query method.
                let query_fn_ident =
                    syn::Ident::new(&format!("query_{}", field_ident), field_ident.span());
                let query_method = quote! {
                    /// Query the web element from the DOM.
                    ///
                    /// Returns `Some(WebElement)` if found, `None` otherwise.
                    pub async fn #query_fn_ident(&self, driver: &thirtyfour::WebDriver) -> Option<thirtyfour::WebElement> {
                        match driver.query(self.#field_ident.clone()).first_opt().await {
                            Ok(Some(element)) => Some(element),
                            Ok(None) => None,
                            Err(e) => {
                                log::error!("Error querying element {}: {}", #field_name_str, e);
                                None
                            }
                        }
                    }
                };
                methods.push(query_method);

                // Try to parse any extra methods from the attribute.
                let mut extra_methods = Vec::new();
                for attr in &field.attrs {
                    if attr.path().is_ident("thirtyfour_actions") {
                        match attr.parse_args::<ElementMethods>() {
                            Ok(parsed) => {
                                extra_methods.extend(parsed.methods);
                            }
                            Err(e) => {
                                return syn::Error::new(
                                    attr.span(),
                                    format!("Failed to parse thirtyfour_actions attribute: {}", e),
                                )
                                .to_compile_error()
                                .into();
                            }
                        }
                    }
                }

                // For each extra method requested, generate its implementation.
                for extra in extra_methods {
                    match extra.as_str() {
                        "click" => {
                            let click_fn_ident = syn::Ident::new(
                                &format!("click_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Click on the web element.
                                pub async fn #click_fn_ident(&self, driver: &thirtyfour::WebDriver) -> anyhow::Result<()> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            element.click().await
                                                .map_err(|e| anyhow::anyhow!("Failed to click {}: {}", #field_name_str, e))?;
                                            Ok(())
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
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
                                /// Enter text into the web element.
                                pub async fn #enter_fn_ident(&self, driver: &thirtyfour::WebDriver, keys: &str) -> anyhow::Result<()> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(input) => {
                                            input.send_keys(keys).await
                                                .map_err(|e| anyhow::anyhow!("Failed to send keys to {}: {}", #field_name_str, e))?;
                                            Ok(())
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }
                        "get_text" => {
                            let get_text_fn_ident = syn::Ident::new(
                                &format!("get_text_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Get the text content of the web element.
                                pub async fn #get_text_fn_ident(&self, driver: &thirtyfour::WebDriver) -> anyhow::Result<String> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            element.text().await
                                                .map_err(|e| anyhow::anyhow!("Failed to get text from {}: {}", #field_name_str, e))
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }
                        "is_displayed" => {
                            let is_displayed_fn_ident = syn::Ident::new(
                                &format!("is_displayed_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Check if the web element is displayed.
                                pub async fn #is_displayed_fn_ident(&self, driver: &thirtyfour::WebDriver) -> anyhow::Result<bool> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            element.is_displayed().await
                                                .map_err(|e| anyhow::anyhow!("Failed to check if {} is displayed: {}", #field_name_str, e))
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }
                        "is_selected" => {
                            let is_selected_fn_ident = syn::Ident::new(
                                &format!("is_selected_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Check if the web element is selected.
                                pub async fn #is_selected_fn_ident(&self, driver: &thirtyfour::WebDriver) -> anyhow::Result<bool> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            element.is_selected().await
                                                .map_err(|e| anyhow::anyhow!("Failed to check if {} is selected: {}", #field_name_str, e))
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }
                        "is_enabled" => {
                            let is_enabled_fn_ident = syn::Ident::new(
                                &format!("is_enabled_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Check if the web element is enabled.
                                pub async fn #is_enabled_fn_ident(&self, driver: &thirtyfour::WebDriver) -> anyhow::Result<bool> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            element.is_enabled().await
                                                .map_err(|e| anyhow::anyhow!("Failed to check if {} is enabled: {}", #field_name_str, e))
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }
                        "clear" => {
                            let clear_fn_ident = syn::Ident::new(
                                &format!("clear_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Clear input field content.
                                pub async fn #clear_fn_ident(&self, driver: &thirtyfour::WebDriver) -> anyhow::Result<()> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            element.clear().await
                                                .map_err(|e| anyhow::anyhow!("Failed to clear {}: {}", #field_name_str, e))?;
                                            Ok(())
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }
                        _ => {
                            return syn::Error::new(
                                field_ident.span(),
                                format!(
                                    "Unsupported thirtyfour_actions method: '{}' for field {}",
                                    extra, field_name_str
                                ),
                            )
                            .to_compile_error()
                            .into();
                        }
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
