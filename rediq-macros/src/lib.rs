//! Proc macros for rediq task handlers
//!
//! This crate provides attribute and derive macros to simplify task handler registration.

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, ItemFn, LitStr, Token};

/// Attribute macro to convert an async function into a task handler
///
/// The macro implements the Handler trait for the function, allowing it to be used
/// with the Mux router.
///
/// # Example
///
/// ```rust,no_run
/// use rediq::{Task, Result};
/// use rediq_macros::task_handler;
///
/// #[task_handler]
/// async fn send_email(task: &Task) -> Result<()> {
///     println!("Processing email task: {}", task.id);
///     Ok(())
/// }
///
/// // Later, register the handler:
/// let mut mux = rediq::processor::Mux::new();
/// mux.handle("email:send", send_email);
/// ```
#[proc_macro_attribute]
pub fn task_handler(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(input as ItemFn);

    let fn_name = &input_fn.sig.ident;
    let fn_vis = &input_fn.vis;
    let fn_attrs = &input_fn.attrs;
    let fn_inputs = &input_fn.sig.inputs;
    let fn_output = &input_fn.sig.output;
    let fn_block = &input_fn.block;

    // Validate function signature
    if input_fn.sig.asyncness.is_none() {
        return syn::Error::new_spanned(
            &input_fn.sig.fn_token,
            "task handler must be an async function",
        )
        .to_compile_error()
        .into();
    }

    // Generate a wrapper struct that implements Handler
    let wrapper_name = syn::Ident::new(&format!("{}Wrapper", fn_name), fn_name.span());

    let expanded = quote! {
        #(#fn_attrs)*
        #fn_vis fn #fn_name #fn_inputs #fn_output {
            #fn_block
        }

        #[automatically_derived]
        #[allow(non_camel_case_types)]
        #fn_vis struct #wrapper_name;

        #[async_trait::async_trait]
        impl rediq::processor::Handler for #wrapper_name {
            async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
                #fn_name(task).await
            }
        }

        // Implement From to allow easy conversion
        #[automatically_derived]
        impl From<#wrapper_name> for std::sync::Arc<dyn rediq::processor::Handler> {
            fn from(_: #wrapper_name) -> Self {
                std::sync::Arc::new(#wrapper_name)
            }
        }

        // Implement Default for the wrapper
        #[automatically_derived]
        impl Default for #wrapper_name {
            fn default() -> Self {
                #wrapper_name
            }
        }
    };

    TokenStream::from(expanded)
}

/// Macro to register multiple task handlers at once
///
/// # Syntax
///
/// ```ignore
/// register_handlers!(
///     "task_type1" => handler_function1,
///     "task_type2" => handler_function2,
/// )
/// ```
///
/// # Example
///
/// ```rust,no_run
/// use rediq::{Mux, Result};
/// use rediq_macros::{register_handlers, task_handler};
///
/// #[task_handler]
/// async fn send_email(task: &rediq::Task) -> Result<()> {
///     Ok(())
/// }
///
/// #[task_handler]
/// async fn send_sms(task: &rediq::Task) -> Result<()> {
///     Ok(())
/// }
///
/// let mux = register_handlers!(
///     "email:send" => send_email,
///     "sms:send" => send_sms,
/// );
/// ```
#[proc_macro]
pub fn register_handlers(input: TokenStream) -> TokenStream {
    struct HandlerList {
        mappings: Vec<(LitStr, syn::Ident)>,
    }

    impl Parse for HandlerList {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let mut mappings = Vec::new();

            while !input.is_empty() {
                // Parse task type (literal)
                let task_type: LitStr = input.parse()?;

                // Parse =>
                input.parse::<Token![=>]>()?;

                // Parse handler function name
                let handler: syn::Ident = input.parse()?;

                mappings.push((task_type, handler));

                // Parse comma if not at end
                if !input.is_empty() {
                    input.parse::<Token![,]>()?;
                }
            }

            Ok(HandlerList { mappings })
        }
    }

    let handler_list = parse_macro_input!(input as HandlerList);

    let register_calls = handler_list.mappings.iter().map(|(task_type, handler)| {
        let wrapper_name = syn::Ident::new(&format!("{}Wrapper", handler), handler.span());
        quote! {
            mux.handle(#task_type, #wrapper_name);
        }
    });

    let expanded = quote! {
        {
            let mut mux = rediq::processor::Mux::new();
            #(#register_calls)*
            mux
        }
    };

    TokenStream::from(expanded)
}

/// Helper macro to create a handler from an async function inline
///
/// This is useful when you don't want to use the attribute macro.
///
/// # Example
///
/// ```rust,no_run
/// use rediq::{Mux, Task, Result};
/// use rediq_macros::handler_fn;
///
/// async fn my_handler(task: &Task) -> Result<()> {
///     Ok(())
/// }
///
/// let mut mux = Mux::new();
/// mux.handle("my:task", handler_fn!(my_handler));
/// ```
#[proc_macro]
pub fn handler_fn(input: TokenStream) -> TokenStream {
    let handler_fn = parse_macro_input!(input as syn::Ident);
    let wrapper_name = syn::Ident::new(&format!("{}Wrapper", handler_fn), handler_fn.span());

    let expanded = quote! {
        #wrapper_name
    };

    TokenStream::from(expanded)
}

/// Convenience macro to define a handler inline
///
/// This macro allows you to define a handler function and get its wrapper
/// in a single expression.
///
/// # Example
///
/// ```rust,no_run
/// use rediq::{Mux, Task, Result};
/// use rediq_macros::def_handler;
///
/// let mut mux = Mux::new();
/// mux.handle("my:task", def_handler!(async fn(task: &Task) -> Result<()> {
///     println!("Handling task: {}", task.id);
///     Ok(())
/// }));
/// ```
#[proc_macro]
pub fn def_handler(input: TokenStream) -> TokenStream {
    let handler_fn = parse_macro_input!(input as ItemFn);
    let fn_name = &handler_fn.sig.ident;

    let wrapper_name = syn::Ident::new(&format!("{}Wrapper", fn_name), fn_name.span());

    let expanded = quote! {
        #handler_fn

        #[allow(non_camel_case_types)]
        struct #wrapper_name;

        #[async_trait::async_trait]
        impl rediq::processor::Handler for #wrapper_name {
            async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
                #fn_name(task).await
            }
        }

        #wrapper_name
    };

    TokenStream::from(expanded)
}
