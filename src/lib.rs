extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::Ident;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{DeriveInput, parse_macro_input, spanned::Spanned};

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
///     #[thirtyfour_actions(methods(click, enter_keys, get_text, etc))]
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
                        // Basic element interactions
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
                        "double_click" => {
                            let double_click_fn_ident = syn::Ident::new(
                                &format!("double_click_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Double-click on the web element.
                                pub async fn #double_click_fn_ident(&self, driver: &thirtyfour::WebDriver) -> anyhow::Result<()> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            let actions = driver.action_chain();
                                            actions.double_click(&element).perform().await
                                                .map_err(|e| anyhow::anyhow!("Failed to double-click {}: {}", #field_name_str, e))?;
                                            Ok(())
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }
                        "right_click" => {
                            let right_click_fn_ident = syn::Ident::new(
                                &format!("right_click_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Right-click (context click) on the web element.
                                pub async fn #right_click_fn_ident(&self, driver: &thirtyfour::WebDriver) -> anyhow::Result<()> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            let actions = driver.action_chain();
                                            actions.context_click(&element).perform().await
                                                .map_err(|e| anyhow::anyhow!("Failed to right-click {}: {}", #field_name_str, e))?;
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
                        "submit" => {
                            let submit_fn_ident = syn::Ident::new(
                                &format!("submit_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Submit a form element.
                                pub async fn #submit_fn_ident(&self, driver: &thirtyfour::WebDriver) -> anyhow::Result<()> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            element.submit().await
                                                .map_err(|e| anyhow::anyhow!("Failed to submit form {}: {}", #field_name_str, e))?;
                                            Ok(())
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }
                        "hover" => {
                            let hover_fn_ident = syn::Ident::new(
                                &format!("hover_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Hover over the web element (move mouse to it).
                                pub async fn #hover_fn_ident(&self, driver: &thirtyfour::WebDriver) -> anyhow::Result<()> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            let actions = driver.action_chain();
                                            actions.move_to_element(&element).perform().await
                                                .map_err(|e| anyhow::anyhow!("Failed to hover over {}: {}", #field_name_str, e))?;
                                            Ok(())
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }
                        "drag_to" => {
                            let drag_to_fn_ident = syn::Ident::new(
                                &format!("drag_{}_to", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Drag this element to another target element.
                                pub async fn #drag_to_fn_ident(&self, driver: &thirtyfour::WebDriver, target_element: &thirtyfour::WebElement) -> anyhow::Result<()> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            let actions = driver.action_chain();
                                            actions.drag_and_drop(&element, target_element).perform().await
                                                .map_err(|e| anyhow::anyhow!("Failed to drag {} to target: {}", #field_name_str, e))?;
                                            Ok(())
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }

                        // Element properties and state
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
                        "get_attribute" => {
                            let get_attr_fn_ident = syn::Ident::new(
                                &format!("get_attribute_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Get a specific attribute value from the web element.
                                pub async fn #get_attr_fn_ident(&self, driver: &thirtyfour::WebDriver, attribute: &str) -> anyhow::Result<Option<String>> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            element.attr(attribute).await
                                                .map_err(|e| anyhow::anyhow!("Failed to get attribute '{}' from {}: {}",
                                                    attribute, #field_name_str, e))
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }
                        "get_value" => {
                            let get_value_fn_ident = syn::Ident::new(
                                &format!("get_value_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Get the value attribute of a form control element.
                                pub async fn #get_value_fn_ident(&self, driver: &thirtyfour::WebDriver) -> anyhow::Result<Option<String>> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            element.attr("value").await
                                                .map_err(|e| anyhow::anyhow!("Failed to get value from {}: {}", #field_name_str, e))
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }
                        "get_css_value" => {
                            let get_css_fn_ident = syn::Ident::new(
                                &format!("get_css_value_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Get a CSS property value of the web element.
                                pub async fn #get_css_fn_ident(&self, driver: &thirtyfour::WebDriver, property: &str) -> anyhow::Result<String> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            element.css_value(property).await
                                                .map_err(|e| anyhow::anyhow!("Failed to get CSS property '{}' from {}: {}",
                                                    property, #field_name_str, e))
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }
                        "has_class" => {
                            let has_class_fn_ident = syn::Ident::new(
                                &format!("has_class_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Check if the element has a specific CSS class.
                                pub async fn #has_class_fn_ident(&self, driver: &thirtyfour::WebDriver, class_name: &str) -> anyhow::Result<bool> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            let class_attr = element.attr("class").await
                                                .map_err(|e| anyhow::anyhow!("Failed to get class attribute from {}: {}", #field_name_str, e))?;

                                            match class_attr {
                                                Some(classes) => {
                                                    let class_list: Vec<&str> = classes.split_whitespace().collect();
                                                    Ok(class_list.contains(&class_name))
                                                },
                                                None => Ok(false)
                                            }
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }

                        // Element state checks
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
                        "exists" => {
                            let exists_fn_ident = syn::Ident::new(
                                &format!("exists_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Check if the element exists in the DOM without throwing an error.
                                pub async fn #exists_fn_ident(&self, driver: &thirtyfour::WebDriver) -> bool {
                                    match driver.query(self.#field_ident.clone()).exists().await {
                                        Ok(exists) => exists,
                                        Err(_) => false
                                    }
                                }
                            };
                            methods.push(method);
                        }

                        // Select element methods
                        "select_by_text" => {
                            let select_text_fn_ident = syn::Ident::new(
                                &format!("select_by_text_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Select an option from a dropdown by its visible text.
                                pub async fn #select_text_fn_ident(&self, driver: &thirtyfour::WebDriver, text: &str) -> anyhow::Result<()> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            let select = thirtyfour::components::select::SelectElement::new(&element);
                                            select.select_by_visible_text(text).await
                                                .map_err(|e| anyhow::anyhow!("Failed to select text '{}' in {}: {}", text, #field_name_str, e))?;
                                            Ok(())
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }
                        "select_by_value" => {
                            let select_value_fn_ident = syn::Ident::new(
                                &format!("select_by_value_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Select an option from a dropdown by its value attribute.
                                pub async fn #select_value_fn_ident(&self, driver: &thirtyfour::WebDriver, value: &str) -> anyhow::Result<()> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            let select = thirtyfour::components::select::SelectElement::new(&element);
                                            select.select_by_value(value).await
                                                .map_err(|e| anyhow::anyhow!("Failed to select value '{}' in {}: {}", value, #field_name_str, e))?;
                                            Ok(())
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }
                        "select_by_index" => {
                            let select_index_fn_ident = syn::Ident::new(
                                &format!("select_by_index_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Select an option from a dropdown by its index.
                                pub async fn #select_index_fn_ident(&self, driver: &thirtyfour::WebDriver, index: usize) -> anyhow::Result<()> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            let select = thirtyfour::components::select::SelectElement::new(&element);
                                            select.select_by_index(index).await
                                                .map_err(|e| anyhow::anyhow!("Failed to select index {} in {}: {}", index, #field_name_str, e))?;
                                            Ok(())
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }
                        "get_selected_text" => {
                            let get_selected_fn_ident = syn::Ident::new(
                                &format!("get_selected_text_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Get the text of the currently selected option in a dropdown.
                                pub async fn #get_selected_fn_ident(&self, driver: &thirtyfour::WebDriver) -> anyhow::Result<String> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            let select = thirtyfour::components::select::SelectElement::new(&element);
                                            select.first_selected_option().await
                                                .map_err(|e| anyhow::anyhow!("Failed to get selected option in {}: {}", #field_name_str, e))?
                                                .text().await
                                                .map_err(|e| anyhow::anyhow!("Failed to get text of selected option in {}: {}", #field_name_str, e))
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }

                        // Visibility and waiting methods
                        "scroll_to" => {
                            let scroll_fn_ident = syn::Ident::new(
                                &format!("scroll_to_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Scroll the element into view.
                                pub async fn #scroll_fn_ident(&self, driver: &thirtyfour::WebDriver) -> anyhow::Result<()> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            driver.execute(r#"arguments[0].scrollIntoView();"#, vec![element.clone().into()]).await
                                                .map_err(|e| anyhow::anyhow!("Failed to scroll to {}: {}", #field_name_str, e))?;
                                            Ok(())
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }
                        "wait_for" => {
                            let wait_fn_ident = syn::Ident::new(
                                &format!("wait_for_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Wait for the element to be present and visible with timeout.
                                pub async fn #wait_fn_ident(&self, driver: &thirtyfour::WebDriver, timeout_secs: u64) -> anyhow::Result<thirtyfour::WebElement> {
                                    use std::time::Duration;
                                    driver.query(self.#field_ident.clone())
                                        .wait(Duration::from_secs(timeout_secs), Duration::from_millis(500))
                                        .visible()
                                        .first()
                                        .await
                                        .map_err(|e| anyhow::anyhow!("Timed out waiting for {} to be visible: {}", #field_name_str, e))
                                }
                            };
                            methods.push(method);
                        }
                        "wait_until_clickable" => {
                            let wait_clickable_fn_ident = syn::Ident::new(
                                &format!("wait_until_clickable_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Wait until the element is clickable (visible and enabled).
                                pub async fn #wait_clickable_fn_ident(&self, driver: &thirtyfour::WebDriver, timeout_secs: u64) -> anyhow::Result<thirtyfour::WebElement> {
                                    use std::time::Duration;
                                    let element = driver.query(self.#field_ident.clone())
                                        .wait(Duration::from_secs(timeout_secs), Duration::from_millis(500))
                                        .visible()
                                        .first()
                                        .await
                                        .map_err(|e| anyhow::anyhow!("Timed out waiting for {} to be visible: {}", #field_name_str, e))?;

                                    // Check if enabled
                                    if !element.is_enabled().await
                                        .map_err(|e| anyhow::anyhow!("Failed to check if {} is enabled: {}", #field_name_str, e))? {
                                        return Err(anyhow::anyhow!("Element {} is not clickable (disabled)", #field_name_str));
                                    }

                                    Ok(element)
                                }
                            };
                            methods.push(method);
                        }
                        "take_screenshot" => {
                            let screenshot_fn_ident = syn::Ident::new(
                                &format!("take_screenshot_{}", field_ident),
                                field_ident.span(),
                            );
                            let method = quote! {
                                /// Take a screenshot of just this element and return the PNG image data as base64.
                                pub async fn #screenshot_fn_ident(&self, driver: &thirtyfour::WebDriver) -> anyhow::Result<String> {
                                    match self.#query_fn_ident(driver).await {
                                        Some(element) => {
                                            element.screenshot_as_base64().await
                                                .map_err(|e| anyhow::anyhow!("Failed to take screenshot of {}: {}", #field_name_str, e))
                                        },
                                        None => Err(anyhow::anyhow!("Element {} not found", #field_name_str))
                                    }
                                }
                            };
                            methods.push(method);
                        }

                        // If the method isn't supported, generate a compile-time error
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
