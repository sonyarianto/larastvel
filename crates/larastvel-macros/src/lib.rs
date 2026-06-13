use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, ItemStruct};

#[proc_macro_derive(Resource)]
pub fn derive_resource(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        impl #name {
            pub fn routes() -> Vec<(&'static str, &'static str)> {
                vec![]
            }
        }

        impl larastvel_core::routing::ResourceController for #name
        where
            #name: Clone + Send + Sync + 'static,
        {
            fn index(&self) -> impl larastvel_core::axum::response::IntoResponse {
                larastvel_core::axum::response::Json(serde_json::json!({"data": []}))
            }

            fn create(&self) -> impl larastvel_core::axum::response::IntoResponse {
                larastvel_core::axum::response::Json(serde_json::json!({"data": {}}))
            }

            fn store(&self) -> impl larastvel_core::axum::response::IntoResponse {
                larastvel_core::axum::response::Json(serde_json::json!({"data": {}}))
            }

            async fn show(&self, _id: String) -> impl larastvel_core::axum::response::IntoResponse {
                larastvel_core::axum::response::Json(serde_json::json!({"data": {}}))
            }

            async fn edit(&self, _id: String) -> impl larastvel_core::axum::response::IntoResponse {
                larastvel_core::axum::response::Json(serde_json::json!({"data": {}}))
            }

            async fn update(&self, _id: String) -> impl larastvel_core::axum::response::IntoResponse {
                larastvel_core::axum::response::Json(serde_json::json!({"data": {}}))
            }

            async fn destroy(&self, _id: String) -> impl larastvel_core::axum::response::IntoResponse {
                larastvel_core::axum::response::Json(serde_json::json!({"data": {}}))
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn controller(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let name = &input.ident;

    let expanded = quote! {
        #input

        impl #name {
            pub fn register_routes(router: &larastvel_core::routing::Registrar) {
                let ctrl = #name {};
                router.resource(stringify!(#name).to_lowercase().replace("controller", ""), ctrl);
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn route(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
