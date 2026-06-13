use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, ItemStruct};

fn resource_name_from_ident(name: &syn::Ident) -> String {
    let s = name.to_string();
    s.strip_suffix("Controller")
        .unwrap_or(&s)
        .to_lowercase()
}

#[proc_macro_derive(Resource, attributes(resource))]
pub fn derive_resource(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
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

#[proc_macro_attribute]
pub fn route(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
