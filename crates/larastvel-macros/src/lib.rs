use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, ImplItem, ItemImpl, ItemStruct, Lit,
};

fn resource_name_from_ident(name: &syn::Ident) -> String {
    let s = name.to_string();
    s.strip_suffix("Controller").unwrap_or(&s).to_lowercase()
}

// ---------------------------------------------------------------------------
// Derive: Resource
// ---------------------------------------------------------------------------

#[proc_macro_derive(Resource, attributes(resource))]
pub fn derive_resource(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let name = &input.ident;

    let resource_name = resource_name_from_ident(name);

    let expanded = quote! {
        impl larastvel_core::routing::ResourceController for #name {
            const RESOURCE_NAME: &'static str = #resource_name;
        }

        impl #name {
            pub fn register_routes(
                registrar: &larastvel_core::routing::Registrar,
            ) {
                let __name =
                    <Self as larastvel_core::routing::ResourceController>::RESOURCE_NAME;

                registrar.get(
                    &format!("/{}", __name),
                    Self::__resource_index,
                );
                registrar.get(
                    &format!("/{}/create", __name),
                    Self::__resource_create,
                );
                registrar.post(
                    &format!("/{}", __name),
                    Self::__resource_store,
                );
                registrar.get(
                    &format!("/{}/{{id}}", __name),
                    Self::__resource_show,
                );
                registrar.get(
                    &format!("/{}/{{id}}/edit", __name),
                    Self::__resource_edit,
                );
                registrar.put(
                    &format!("/{}/{{id}}", __name),
                    Self::__resource_update,
                );
                registrar.delete(
                    &format!("/{}/{{id}}", __name),
                    Self::__resource_destroy,
                );
            }

            async fn __resource_index(
            ) -> larastvel_core::axum::response::Response {
                <Self as larastvel_core::routing::ResourceController>::index().await
            }

            async fn __resource_create(
            ) -> larastvel_core::axum::response::Response {
                <Self as larastvel_core::routing::ResourceController>::create().await
            }

            async fn __resource_store(
            ) -> larastvel_core::axum::response::Response {
                <Self as larastvel_core::routing::ResourceController>::store().await
            }

            async fn __resource_show(
                larastvel_core::axum::extract::Path(id):
                    larastvel_core::axum::extract::Path<String>,
            ) -> larastvel_core::axum::response::Response {
                <Self as larastvel_core::routing::ResourceController>::show(id).await
            }

            async fn __resource_edit(
                larastvel_core::axum::extract::Path(id):
                    larastvel_core::axum::extract::Path<String>,
            ) -> larastvel_core::axum::response::Response {
                <Self as larastvel_core::routing::ResourceController>::edit(id).await
            }

            async fn __resource_update(
                larastvel_core::axum::extract::Path(id):
                    larastvel_core::axum::extract::Path<String>,
            ) -> larastvel_core::axum::response::Response {
                <Self as larastvel_core::routing::ResourceController>::update(id).await
            }

            async fn __resource_destroy(
                larastvel_core::axum::extract::Path(id):
                    larastvel_core::axum::extract::Path<String>,
            ) -> larastvel_core::axum::response::Response {
                <Self as larastvel_core::routing::ResourceController>::destroy(id).await
            }
        }
    };

    TokenStream::from(expanded)
}

// ---------------------------------------------------------------------------
// Attribute: #[controller]
// ---------------------------------------------------------------------------

#[proc_macro_attribute]
pub fn controller(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let name = &input.ident;

    let expanded = quote! {
        #input

        impl #name {
            pub fn register_routes(
                _registrar: &larastvel_core::routing::Registrar,
            ) {
            }
        }
    };

    TokenStream::from(expanded)
}

// ---------------------------------------------------------------------------
// Helper: string literal argument parser
// ---------------------------------------------------------------------------

struct UriArg {
    uri: String,
}

impl Parse for UriArg {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lit: Lit = input.parse()?;
        match lit {
            Lit::Str(s) => Ok(UriArg { uri: s.value() }),
            _ => Err(syn::Error::new(lit.span(), "expected string literal")),
        }
    }
}

// ---------------------------------------------------------------------------
// Marker attribute macros: #[get], #[post], #[put], #[patch], #[delete], #[ws]
// These are identity transforms.  The #[route] macro on the enclosing impl
// block reads them out of the raw attribute list.
// ---------------------------------------------------------------------------

/// Marks a handler function for GET requests. Must be used inside a `#[route]` impl block.
#[proc_macro_attribute]
pub fn get(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a handler function for POST requests. Must be used inside a `#[route]` impl block.
#[proc_macro_attribute]
pub fn post(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a handler function for PUT requests. Must be used inside a `#[route]` impl block.
#[proc_macro_attribute]
pub fn put(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a handler function for PATCH requests. Must be used inside a `#[route]` impl block.
#[proc_macro_attribute]
pub fn patch(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a handler function for DELETE requests. Must be used inside a `#[route]` impl block.
#[proc_macro_attribute]
pub fn delete(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a handler function for WebSocket upgrades. Must be used inside a `#[route]` impl block.
#[proc_macro_attribute]
pub fn ws(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

// ---------------------------------------------------------------------------
// Attribute: #[route]  (on impl blocks)
//
// Scans method-level #[get("…")], #[post("…")] etc. markers and generates
// a `register_routes(&Registrar)` method.
//
// Usage:
//   #[route]
//   impl MyController {
//       #[get("/users")]
//       async fn index() -> impl IntoResponse { … }
//
//       #[post("/users")]
//       async fn store(Json(body): Json<Input>) -> impl IntoResponse { … }
//   }
// ---------------------------------------------------------------------------

#[proc_macro_attribute]
pub fn route(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item_str = item.to_string();

    // Only process impl blocks.
    if !item_str.starts_with("impl") {
        return item;
    }

    let input: ItemImpl = syn::parse(item.clone()).unwrap();
    let self_ty = input.self_ty.clone();
    let mut registrations = Vec::new();
    let mut clean_fns = Vec::new();

    // First pass: collect registrations and rebuild clean methods.
    for method_item in &input.items {
        if let ImplItem::Fn(m) = method_item {
            let mut method_name: Option<String> = None;
            let mut uri: Option<String> = None;

            for attr in &m.attrs {
                if let Some(ident) = attr.path().get_ident() {
                    let name = ident.to_string();
                    if matches!(
                        name.as_str(),
                        "get" | "post" | "put" | "patch" | "delete" | "ws"
                    ) {
                        let parsed: UriArg = attr.parse_args().unwrap();
                        method_name = Some(name.to_uppercase());
                        uri = Some(parsed.uri);
                    }
                }
            }

            let fn_name = &m.sig.ident;
            let (attrs, vis, sig, block) = (&m.attrs, &m.vis, &m.sig, &m.block);

            // Keep attributes that are NOT route markers.
            let kept_attrs: Vec<_> = attrs
                .iter()
                .filter(|a| {
                    a.path().get_ident().map(|i| i.to_string()).is_none_or(|n| {
                        !matches!(
                            n.as_str(),
                            "get" | "post" | "put" | "patch" | "delete" | "ws"
                        )
                    })
                })
                .collect();

            clean_fns.push(quote! {
                #(#kept_attrs)*
                #vis #sig #block
            });

            if let (Some(method), Some(uri)) = (method_name, uri) {
                let method_ident =
                    syn::Ident::new(&method.to_lowercase(), proc_macro2::Span::call_site());
                let registrar_call = if method == "WS" {
                    quote! { registrar.ws(#uri, Self::#fn_name); }
                } else {
                    quote! { registrar.#method_ident(#uri, Self::#fn_name); }
                };
                registrations.push(registrar_call);
            }
        }
    }

    let gen = quote! {
        impl #self_ty {
            #(#clean_fns)*
        }

        impl #self_ty {
            pub fn register_routes(registrar: &larastvel_core::routing::Registrar) {
                #(#registrations)*
            }
        }
    };

    TokenStream::from(gen)
}
