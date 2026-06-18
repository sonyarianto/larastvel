use proc_macro::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, ImplItem, ItemImpl, ItemStruct, Lit,
};

fn snake_to_pascal(s: &str) -> String {
    s.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().to_string() + c.as_str(),
            }
        })
        .collect()
}

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

/// Marks a handler function with middleware. Must be used inside a `#[route]` impl block.
///
/// Accepts one or more middleware names:
/// ```ignore
/// #[middleware("auth")]
/// #[middleware("auth", "admin")]
/// ```
#[proc_macro_attribute]
pub fn middleware(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

// ---------------------------------------------------------------------------
// Parser for middleware arguments: one or more comma-separated string literals.
// ---------------------------------------------------------------------------

struct MiddlewareArgs {
    names: Vec<String>,
}

impl Parse for MiddlewareArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut names = Vec::new();
        while !input.is_empty() {
            let lit: Lit = input.parse()?;
            match lit {
                Lit::Str(s) => names.push(s.value()),
                _ => return Err(syn::Error::new(lit.span(), "expected string literal")),
            }
            if !input.is_empty() {
                input.parse::<syn::Token![,]>()?;
            }
        }
        Ok(MiddlewareArgs { names })
    }
}

// ---------------------------------------------------------------------------
// Attribute: #[listener(EventType)]
//
// Converts an async function into a zero-sized Listener struct that
// implements the `Listener<EventType>` trait and provides a `listen()`
// helper to register it with `EventService`.
//
// Usage:
//   #[listener(OrderShipped)]
//   async fn send_notification(event: OrderShipped) {
//       tracing::info!("Order {} shipped", event.order_id);
//   }
//
// Expands to:
//   struct SendNotificationListener;
//
//   #[async_trait]
//   impl listener<OrderShipped> for SendNotificationListener {
//       async fn handle(&self, event: OrderShipped) { ... }
//   }
//
//   impl SendNotificationListener {
//       fn listen() {
//           EventService::listen::<OrderShipped, Self>(Self);
//       }
//   }
// ---------------------------------------------------------------------------

#[proc_macro_attribute]
pub fn listener(attr: TokenStream, item: TokenStream) -> TokenStream {
    let event_ty: syn::Type = parse_macro_input!(attr as syn::Type);
    let func = parse_macro_input!(item as syn::ItemFn);

    if func.sig.asyncness.is_none() {
        return syn::Error::new(func.sig.span(), "#[listener] requires async fn")
            .to_compile_error()
            .into();
    }

    let fn_name = &func.sig.ident;
    let struct_name_str = snake_to_pascal(&fn_name.to_string()) + "Listener";
    let struct_name = syn::Ident::new(&struct_name_str, fn_name.span());
    let vis = &func.vis;
    let body = &func.block;
    let params = &func.sig.inputs;

    let expanded = quote! {
        #vis struct #struct_name;

        #[larastvel_core::async_trait]
        impl larastvel_core::events::Listener<#event_ty> for #struct_name {
            async fn handle(&self, #params) {
                #body
            }
        }

        impl #struct_name {
            pub fn listen() {
                larastvel_core::events::EventService::listen::<#event_ty, Self>(Self);
            }
        }
    };

    TokenStream::from(expanded)
}

// ---------------------------------------------------------------------------
// Attribute: #[job]
//
// Converts an async function into a queue job struct that implements
// `ShouldQueue`.  The function parameters become struct fields, and the
// generated struct provides `new()`, `dispatch()`, and `name()` methods.
//
// Usage:
//   #[job]
//   async fn process_podcast(podcast_id: i32) -> Result<(), JobError> {
//       tracing::info!("Processing podcast {}", podcast_id);
//       Ok(())
//   }
//
// Expands to:
//   #[derive(Debug)]
//   pub struct ProcessPodcastJob {
//       pub podcast_id: i32,
//   }
//
//   impl ProcessPodcastJob {
//       pub fn new(podcast_id: i32) -> Self { ... }
//       pub async fn dispatch(self) -> Result<(), JobError> { ... }
//   }
//
//   #[async_trait]
//   impl ShouldQueue for ProcessPodcastJob {
//       fn name(&self) -> &str { "process_podcast" }
//       async fn handle(&self) -> Result<(), JobError> {
//           __process_podcast_inner(self.podcast_id).await
//       }
//   }
//
//   async fn __process_podcast_inner(podcast_id: i32) -> Result<(), JobError> { ... }
// ---------------------------------------------------------------------------

#[proc_macro_attribute]
pub fn job(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as syn::ItemFn);

    if func.sig.asyncness.is_none() {
        return syn::Error::new(func.sig.span(), "#[job] requires async fn")
            .to_compile_error()
            .into();
    }

    let fn_name = &func.sig.ident;
    let fn_name_str = fn_name.to_string();
    // Strip trailing "_job" suffix so `fn simple_job` yields struct `SimpleJob`, not `SimpleJobJob`.
    let stem = if fn_name_str.ends_with("_job") && fn_name_str.len() > 4 {
        &fn_name_str[..fn_name_str.len() - 4]
    } else {
        &fn_name_str
    };
    let struct_name_str = snake_to_pascal(stem) + "Job";
    let struct_name = syn::Ident::new(&struct_name_str, fn_name.span());
    let inner_fn_name = syn::Ident::new(&format!("__{}_inner", fn_name_str), fn_name.span());
    let vis = &func.vis;
    let output_ty = &func.sig.output;
    let body = &func.block;

    // Parse function params into struct fields.
    let mut field_tokens = Vec::new();
    let mut new_param_tokens = Vec::new();
    let mut field_assignments = Vec::new();
    let mut dispatch_args = Vec::new();
    let mut handle_args = Vec::new();

    for arg in &func.sig.inputs {
        match arg {
            syn::FnArg::Typed(pat_type) => {
                let pat = &pat_type.pat;
                let ty = &pat_type.ty;
                field_tokens.push(quote! { pub #pat: #ty });
                new_param_tokens.push(quote! { #pat: #ty });
                field_assignments.push(quote! { #pat });
                // In dispatch(self), self is owned so we can move fields out.
                dispatch_args.push(quote! { self.#pat });
                // In handle(&self), we must clone since we only have &self.
                handle_args.push(quote! { self.#pat.clone() });
            }
            syn::FnArg::Receiver(_) => {
                return syn::Error::new(arg.span(), "#[job] cannot be used on methods with self")
                    .to_compile_error()
                    .into();
            }
        }
    }

    let expanded = quote! {
        #[derive(Debug)]
        #vis struct #struct_name {
            #(#field_tokens,)*
        }

        impl #struct_name {
            pub fn new(#(#new_param_tokens),*) -> Self {
                Self {
                    #(#field_assignments,)*
                }
            }

            pub async fn dispatch(self) -> Result<(), larastvel_core::queue::JobError> {
                #inner_fn_name(#(#dispatch_args),*).await
            }
        }

        #[larastvel_core::async_trait]
        impl larastvel_core::queue::ShouldQueue for #struct_name {
            fn name(&self) -> &str {
                #fn_name_str
            }

            async fn handle(&self) -> Result<(), larastvel_core::queue::JobError> {
                #inner_fn_name(#(#handle_args),*).await
            }
        }

        #[allow(dead_code)]
        async fn #inner_fn_name(#(#new_param_tokens),*) #output_ty {
            #body
        }
    };

    TokenStream::from(expanded)
}

// ---------------------------------------------------------------------------
// Attribute: #[queued_listener(EventType)]
//
// Like `#[listener]` but dispatches the handler as a background job instead
// of running synchronously.  Generates both a `*Listener` struct (registered
// with EventService) and a `*Job` struct (implements ShouldQueue).
//
// Usage:
//   #[queued_listener(OrderShipped)]
//   async fn handle_order_shipped(event: OrderShipped) -> Result<(), JobError> {
//       tracing::info!("Processing order {}", event.order_id);
//       Ok(())
//   }
//
// This registers a listener that, on each event, dispatches a
// `HandleOrderShippedJob` to the default queue.
// ---------------------------------------------------------------------------

#[proc_macro_attribute]
pub fn queued_listener(attr: TokenStream, item: TokenStream) -> TokenStream {
    let event_ty: syn::Type = parse_macro_input!(attr as syn::Type);
    let func = parse_macro_input!(item as syn::ItemFn);

    if func.sig.asyncness.is_none() {
        return syn::Error::new(func.sig.span(), "#[queued_listener] requires async fn")
            .to_compile_error()
            .into();
    }

    let fn_name = &func.sig.ident;
    let fn_name_str = fn_name.to_string();

    // Strip trailing "_listener" for a cleaner struct name.
    let stem = if fn_name_str.ends_with("_listener") && fn_name_str.len() > 10 {
        &fn_name_str[..fn_name_str.len() - 9]
    } else {
        &fn_name_str
    };
    // Strip trailing "_handler" too.
    let stem = if stem.ends_with("_handler") && stem.len() > 8 {
        &stem[..stem.len() - 8]
    } else {
        stem
    };

    let struct_stem = snake_to_pascal(stem);
    let job_struct_name = syn::Ident::new(&(struct_stem.clone() + "Job"), fn_name.span());
    let listener_struct_name = syn::Ident::new(&(struct_stem + "Listener"), fn_name.span());
    let inner_fn_name = syn::Ident::new(&format!("__{}_inner", fn_name_str), fn_name.span());
    let vis = &func.vis;
    let output = &func.sig.output;
    let body = &func.block;

    // Extract the event parameter.
    let event_param = func.sig.inputs.first().and_then(|arg| match arg {
        syn::FnArg::Typed(pat_type) => Some((pat_type.pat.clone(), pat_type.ty.clone())),
        _ => None,
    });

    let Some((event_pat, event_ty_inner)) = event_param else {
        return syn::Error::new(
            func.sig.span(),
            "#[queued_listener] requires exactly one parameter: the event",
        )
        .to_compile_error()
        .into();
    };

    let expanded = quote! {
        // --- Job struct ---
        #[derive(Debug)]
        #vis struct #job_struct_name {
            pub #event_pat: #event_ty_inner,
        }

        impl #job_struct_name {
            pub fn new(#event_pat: #event_ty_inner) -> Self {
                Self { #event_pat }
            }

            pub async fn dispatch(self) -> Result<(), larastvel_core::queue::JobError> {
                #inner_fn_name(self.#event_pat).await
            }
        }

        #[larastvel_core::async_trait]
        impl larastvel_core::queue::ShouldQueue for #job_struct_name {
            fn name(&self) -> &str {
                #fn_name_str
            }

            async fn handle(&self) -> Result<(), larastvel_core::queue::JobError> {
                #inner_fn_name(self.#event_pat.clone()).await
            }
        }

        #[allow(dead_code)]
        async fn #inner_fn_name(#event_pat: #event_ty_inner) #output {
            #body
        }

        // --- Listener struct ---
        #vis struct #listener_struct_name;

        #[larastvel_core::async_trait]
        impl larastvel_core::events::Listener<#event_ty> for #listener_struct_name {
            async fn handle(&self, #event_pat: #event_ty_inner) {
                if let Err(e) = #job_struct_name::new(#event_pat).dispatch().await {
                    tracing::error!("Queued listener failed: {e}");
                }
            }
        }

        impl #listener_struct_name {
            pub fn listen() {
                larastvel_core::events::EventService::listen::<#event_ty, Self>(Self);
            }
        }
    };

    TokenStream::from(expanded)
}

// ---------------------------------------------------------------------------
// Attribute: #[can("ability")]
//
// Adds authorization checking to an async handler function.
//
// Prepends `AuthenticatedUser` and `Extension<Gate>` extractor parameters
// and wraps the body with an ability check.  The function return type is
// changed to `axum::response::Response`.
//
// Usage:
//   #[can("admin")]
//   async fn dashboard(Extension(state): Extension<AppState>) -> impl IntoResponse {
//       Html("<h1>Admin</h1>")
//   }
// ---------------------------------------------------------------------------

#[proc_macro_attribute]
pub fn can(attr: TokenStream, item: TokenStream) -> TokenStream {
    let parsed = parse_macro_input!(attr as UriArg);
    let ability = parsed.uri;
    let func = parse_macro_input!(item as syn::ItemFn);

    if func.sig.asyncness.is_none() {
        return syn::Error::new(func.sig.span(), "#[can] requires async fn")
            .to_compile_error()
            .into();
    }

    let vis = &func.vis;
    let name = &func.sig.ident;
    let inputs = &func.sig.inputs;
    let body = &func.block;

    for input in inputs {
        if let syn::FnArg::Receiver(_) = input {
            return syn::Error::new(input.span(), "#[can] cannot be used on methods with self")
                .to_compile_error()
                .into();
        }
    }

    let expanded = quote! {
        #vis async fn #name(
            __larastvel_auth: larastvel_core::auth::AuthenticatedUser,
            __larastvel_gate: larastvel_core::axum::Extension<larastvel_core::auth::Gate>,
            #inputs
        ) -> larastvel_core::axum::response::Response {
            if let Err(__larastvel_e) = larastvel_core::auth::check_ability(#ability, &__larastvel_auth, &__larastvel_gate.0).await {
                return __larastvel_e.into_response();
            }
            larastvel_core::axum::response::IntoResponse::into_response(#body)
        }
    };

    TokenStream::from(expanded)
}

// ---------------------------------------------------------------------------
// Attribute: #[validate(rules_expr)]
//
// Validates a JSON request body before the handler runs.
//
// The attribute argument must be an expression that evaluates to
// `Vec<(&str, Vec<Rule>)>`.  The handler must have a `Json<Value>`
// parameter (typically `Json(body): Json<Value>`).
//
// The return type is changed to `axum::response::Response`.
//
// Usage:
//   #[validate(vec![("email", vec![Rule::required(), Rule::email()])])]
//   async fn store(Json(body): Json<Value>) -> impl IntoResponse {
//       Json(json!({"ok": true}))
//   }
// ---------------------------------------------------------------------------

/// Extracts the binding name from a pattern like `Json(body)` or `body`.
fn extract_json_binding(pat: &syn::Pat) -> Option<proc_macro2::TokenStream> {
    match pat {
        // `Json(body)` — body IS the inner Value
        syn::Pat::TupleStruct(ps) if ps.elems.len() == 1 => {
            if let syn::Pat::Ident(inner) = &ps.elems[0] {
                let id = &inner.ident;
                return Some(quote! { #id });
            }
            None
        }
        // `Json { body }` — body IS the inner Value
        syn::Pat::Struct(ps) => ps.fields.first().and_then(|f| match &*f.pat {
            syn::Pat::Ident(ident) => {
                let id = &ident.ident;
                Some(quote! { #id })
            }
            _ => None,
        }),
        // `body: Json<Value>` — body IS the Json<Value> struct, need .0
        syn::Pat::Ident(pi) => {
            let id = &pi.ident;
            Some(quote! { #id . 0 })
        }
        _ => None,
    }
}

#[proc_macro_attribute]
pub fn validate(attr: TokenStream, item: TokenStream) -> TokenStream {
    let rules_expr = proc_macro2::TokenStream::from(attr);
    let func = parse_macro_input!(item as syn::ItemFn);

    if func.sig.asyncness.is_none() {
        return syn::Error::new(func.sig.span(), "#[validate] requires async fn")
            .to_compile_error()
            .into();
    }

    let vis = &func.vis;
    let name = &func.sig.ident;
    let inputs = &func.sig.inputs;
    let body = &func.block;

    for input in inputs {
        if let syn::FnArg::Receiver(_) = input {
            return syn::Error::new(
                input.span(),
                "#[validate] cannot be used on methods with self",
            )
            .to_compile_error()
            .into();
        }
    }

    // Find the Json<Value> binding in the handler parameters.
    let body_access = inputs
        .iter()
        .find_map(|arg| {
            if let syn::FnArg::Typed(pat_type) = arg {
                let ty_str = quote! { #pat_type.ty }.to_string();
                if ty_str.contains("Json") && ty_str.contains("Value") {
                    return extract_json_binding(pat_type.pat.as_ref());
                }
            }
            None
        })
        .unwrap_or_else(|| {
            // Fallback: create a synthetic accessor for "__body.0"
            quote! { __body . 0 }
        });

    let expanded = quote! {
        #vis async fn #name(
            #inputs
        ) -> larastvel_core::axum::response::Response {
            use std::collections::HashMap;

            let __larastvel_body: &serde_json::Value = &(#body_access);
            let __data: HashMap<String, serde_json::Value> = match __larastvel_body.as_object() {
                Some(map) => map.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                None => {
                    let mut __errors = larastvel_core::validation::ValidationErrors::new();
                    __errors.add("_", "Request body must be a JSON object.");
                    return __errors.into_response();
                }
            };

            let __rules = #rules_expr;

            if let Err(__errors) = larastvel_core::validation::validate(&__data, __rules) {
                return __errors.into_response();
            }

            larastvel_core::axum::response::IntoResponse::into_response(#body)
        }
    };

    TokenStream::from(expanded)
}

// ---------------------------------------------------------------------------
// Attribute: #[validated_query(rules_expr)]
//
// Validates query-string parameters before the handler runs.
//
// The attribute argument must be an expression that evaluates to
// `Vec<(&str, Vec<Rule>)>`.  The handler must have a `Query<HashMap<String,
// String>>` parameter (typically `Query(params): Query<HashMap<String, String>>`).
//
// The return type is changed to `axum::response::Response`.
//
// Usage:
//   #[validated_query(vec![("page", vec![required(), numeric()])])]
//   async fn list(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
//       Json(json!({"page": params.get("page")}))
//   }
// ---------------------------------------------------------------------------

/// Extracts the binding name from a pattern like `Query(params)` or `params`.
fn extract_query_binding(pat: &syn::Pat) -> Option<proc_macro2::TokenStream> {
    // Same logic as extract_json_binding — patterns are identical.
    extract_json_binding(pat)
}

#[proc_macro_attribute]
pub fn validated_query(attr: TokenStream, item: TokenStream) -> TokenStream {
    let rules_expr = proc_macro2::TokenStream::from(attr);
    let func = parse_macro_input!(item as syn::ItemFn);

    if func.sig.asyncness.is_none() {
        return syn::Error::new(func.sig.span(), "#[validated_query] requires async fn")
            .to_compile_error()
            .into();
    }

    let vis = &func.vis;
    let name = &func.sig.ident;
    let inputs = &func.sig.inputs;
    let body = &func.block;

    for input in inputs {
        if let syn::FnArg::Receiver(_) = input {
            return syn::Error::new(
                input.span(),
                "#[validated_query] cannot be used on methods with self",
            )
            .to_compile_error()
            .into();
        }
    }

    // Find the Query<…> binding in the handler parameters.
    let body_access = inputs
        .iter()
        .find_map(|arg| {
            if let syn::FnArg::Typed(pat_type) = arg {
                let ty_str = quote! { #pat_type.ty }.to_string();
                if ty_str.contains("Query") {
                    return extract_query_binding(pat_type.pat.as_ref());
                }
            }
            None
        })
        .unwrap_or_else(|| {
            quote! { __params }
        });

    let expanded = quote! {
        #vis async fn #name(
            #inputs
        ) -> larastvel_core::axum::response::Response {
            use std::collections::HashMap;

            let __raw: &HashMap<String, String> = &(#body_access);
            let __data: HashMap<String, serde_json::Value> = __raw
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();

            let __rules = #rules_expr;

            if let Err(__errors) = larastvel_core::validation::validate(&__data, __rules) {
                return __errors.into_response();
            }

            larastvel_core::axum::response::IntoResponse::into_response(#body)
        }
    };

    TokenStream::from(expanded)
}

// ---------------------------------------------------------------------------
// Attribute: #[table("name")]
//
// Converts a plain struct into a full SeaORM entity, eliminating the
// boilerplate of `DeriveEntityModel`, `Relation`, `ActiveModelBehavior`,
// and the `DbModel` wrapper.
//
// Usage:
//   #[table("users")]
//   pub struct User {
//       #[sea_orm(primary_key)]
//       pub id: i32,
//       pub name: String,
//       pub email: String,
//       pub password: String,
//       pub email_verified_at: Option<DateTime>,
//       pub created_at: DateTime,
//       pub updated_at: DateTime,
//   }
//
// Expands to a `__table_User` inner module containing a Model struct
// with `DeriveEntityModel`, a `Relation` enum, `ActiveModelBehavior`,
// and re-exports `Model`, `Entity`, `ActiveModel`, `Column`.
// The original struct name is kept as a `DbModel` wrapper.
//
// Field-level `#[sea_orm(…)]` attributes are passed through directly
// to `DeriveEntityModel` – all SeaORM options (primary_key,
// auto_increment, unique, default_value, indexed, column_type, …)
// are supported.
// ---------------------------------------------------------------------------

#[proc_macro_attribute]
pub fn table(attr: TokenStream, item: TokenStream) -> TokenStream {
    let table_name: syn::LitStr = syn::parse_macro_input!(attr as syn::LitStr);
    let input: syn::ItemStruct = syn::parse_macro_input!(item as syn::ItemStruct);

    let name = &input.ident;
    let vis = &input.vis;
    let fields = &input.fields;
    let table = table_name.value();
    let mod_name = syn::Ident::new(&format!("__table_{}", name), name.span());

    let expanded = quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        mod #mod_name {
            use larastvel_core::sea_orm;
            use sea_orm::entity::prelude::*;

            #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
            #[sea_orm(table_name = #table)]
            pub struct Model #fields

            #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
            pub enum Relation {}

            impl ActiveModelBehavior for ActiveModel {}
        }

        pub use #mod_name::Model;
        pub use #mod_name::Entity;
        pub use #mod_name::ActiveModel;
        pub use #mod_name::Column;

        #vis struct #name;

        impl larastvel_core::models::DbModel for #name {
            type Entity = Entity;
        }
    };

    TokenStream::from(expanded)
}

// ---------------------------------------------------------------------------
// Attribute: #[scope]
//
// Converts a query-scope function into a static method on a DbModel wrapper
// struct (designed for use inside an `impl Model` block).
//
// The first parameter (the SeaORM Select builder) is removed from the
// public signature; the generated method internally calls `Self::query()`
// and passes it as that parameter.
//
// The function name may optionally start with `scope_` — the prefix is
// stripped for the public name (matching Laravel conventions).
//
// Usage:
//   impl User {
//       #[scope]
//       fn popular(query: Select<Entity>, min_likes: i64) -> Select<Entity> {
//           query.filter(Column::Likes.gte(min_likes))
//       }
//   }
//
// Expands to:
//   impl User {
//       pub fn popular(min_likes: i64) -> Select<Entity> {
//           let query = Self::query();
//           query.filter(Column::Likes.gte(min_likes))
//       }
//   }
// ---------------------------------------------------------------------------

#[proc_macro_attribute]
pub fn scope(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as syn::ItemFn);

    // Reject methods that take self.
    for arg in &func.sig.inputs {
        if matches!(arg, syn::FnArg::Receiver(_)) {
            return syn::Error::new(
                func.sig.span(),
                "#[scope] cannot be used on methods with `self`",
            )
            .to_compile_error()
            .into();
        }
    }

    // Require at least one parameter (the query).
    if func.sig.inputs.is_empty() {
        return syn::Error::new(
            func.sig.span(),
            "#[scope] requires at least one parameter (the SeaORM query)",
        )
        .to_compile_error()
        .into();
    }

    let fn_name = &func.sig.ident;
    let name_str = fn_name.to_string();

    // Strip optional "scope_" prefix (Laravel convention).
    let public_name_str = name_str.strip_prefix("scope_").unwrap_or(&name_str);
    let public_name = syn::Ident::new(public_name_str, fn_name.span());

    let vis = &func.vis;
    let output_ty = &func.sig.output;

    // Name of the first parameter (the query builder).
    let first_pat = match func.sig.inputs.first().unwrap() {
        syn::FnArg::Typed(pat_type) => pat_type.pat.as_ref().clone(),
        _ => unreachable!(),
    };

    // Remaining params (everything after the query).
    let remaining: Vec<_> = func.sig.inputs.iter().skip(1).cloned().collect();

    // Prepend `let <query> = Self::query();` to the function body.
    let mut stmts = func.block.stmts;
    let query_stmt: syn::Stmt = syn::parse_quote! {
        let #first_pat = Self::query();
    };
    stmts.insert(0, query_stmt);

    let expanded = quote! {
        #vis fn #public_name(#(#remaining),*) #output_ty {
            #(#stmts)*
        }
    };

    TokenStream::from(expanded)
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
            let mut middleware_names: Vec<String> = Vec::new();

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
                    } else if name == "middleware" {
                        let parsed: MiddlewareArgs = attr.parse_args().unwrap();
                        middleware_names = parsed.names;
                    }
                }
            }

            let fn_name = &m.sig.ident;
            let (attrs, vis, sig, block) = (&m.attrs, &m.vis, &m.sig, &m.block);

            // Keep attributes that are NOT route markers or middleware.
            let kept_attrs: Vec<_> = attrs
                .iter()
                .filter(|a| {
                    a.path().get_ident().map(|i| i.to_string()).is_none_or(|n| {
                        !matches!(
                            n.as_str(),
                            "get" | "post" | "put" | "patch" | "delete" | "ws" | "middleware"
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

                // Set per-route middleware.
                let mw_strings: Vec<_> = middleware_names
                    .iter()
                    .map(|n| proc_macro2::Literal::string(n))
                    .collect();
                let mw_set = if mw_strings.is_empty() {
                    quote! { registrar.with_middleware(Vec::<&str>::new()); }
                } else {
                    quote! { registrar.with_middleware(vec![#(#mw_strings),*]); }
                };

                let registrar_call = if method == "WS" {
                    quote! {
                        #mw_set
                        registrar.ws(#uri, Self::#fn_name);
                    }
                } else {
                    quote! {
                        #mw_set
                        registrar.#method_ident(#uri, Self::#fn_name);
                    }
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

// ---------------------------------------------------------------------------
// Attribute: #[notification]
//
// Generates a Notification trait implementation from an impl block by
// scanning for notification-specific methods (via, to_mail, to_broadcast,
// to_database, to_webhook, to_sms).  Non-notification methods remain on
// the original impl block.
//
// Usage:
// ```rust,ignore
// #[derive(Debug)]
// struct InvoicePaid {
//     invoice_id: i32,
// }
//
// #[notification]
// impl InvoicePaid {
//     fn via(&self) -> Vec<NotificationChannel> {
//         vec![NotificationChannel::Mail]
//     }
//
//     fn to_mail(&self) -> Option<Mailable> {
//         Some(Mailable::html(vec![], "Invoice Paid", "<p>...</p>"))
//     }
// }
// ```
// ---------------------------------------------------------------------------

#[proc_macro_attribute]
pub fn notification(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input: syn::ItemImpl = parse_macro_input!(item as syn::ItemImpl);
    let struct_ty = &input.self_ty;
    let generics = &input.generics;
    let where_clause = &input.generics.where_clause;

    let known_methods: [&str; 6] = [
        "via",
        "to_mail",
        "to_broadcast",
        "to_database",
        "to_webhook",
        "to_sms",
    ];

    let mut notification_impls = Vec::new();
    let mut kept_items = Vec::new();

    for item in input.items {
        if let syn::ImplItem::Fn(method) = &item {
            if known_methods.contains(&method.sig.ident.to_string().as_str()) {
                notification_impls.push(item);
                continue;
            }
        }
        kept_items.push(item);
    }

    let expanded = quote! {
        impl #generics #struct_ty #where_clause {
            #(#kept_items)*
        }

        impl #generics larastvel_core::notifications::Notification for #struct_ty #where_clause {
            #(#notification_impls)*
        }
    };

    TokenStream::from(expanded)
}

// ---------------------------------------------------------------------------
// Attribute: #[observer(Model)]
//
// Generates Listener implementations for model lifecycle events based on
// hook methods defined on an impl block.
//
// The attribute goes on an impl block for an observer struct. Hook methods
// are scanned by name (`created`, `updated`, `deleted`, `saved`,
// `retrieved`) and each generates a `Listener<ModelEvent<M::Entity>>`
// implementation.  An `observe()` method is also generated to register
// all listeners with `EventService`.
//
// Usage:
// ```rust,ignore
// struct UserObserver;
//
// #[observer(User)]
// impl UserObserver {
//     async fn created(&self, model: Model) {
//         tracing::info!("User created: {}", model.email);
//     }
//
//     async fn deleted(&self, model: Model) {
//         tracing::info!("User deleted: {}", model.email);
//     }
// }
//
// // Register the observer at app boot:
// UserObserver::observe();
// ```
// ---------------------------------------------------------------------------

#[proc_macro_attribute]
pub fn observer(attr: TokenStream, item: TokenStream) -> TokenStream {
    let model_ty: syn::Type = parse_macro_input!(attr as syn::Type);
    let input: syn::ItemImpl = parse_macro_input!(item as syn::ItemImpl);

    let struct_ty = &input.self_ty;

    let entity_ty = quote! {
        <#model_ty as larastvel_core::models::DbModel>::Entity
    };
    let event_mod = quote! { larastvel_core::events };
    let listener_trait = quote! { larastvel_core::events::Listener };

    // Known hook method names and their corresponding event type stems
    let hooks: [(&str, &str); 5] = [
        ("created", "ModelCreated"),
        ("updated", "ModelUpdated"),
        ("deleted", "ModelDeleted"),
        ("saved", "ModelSaved"),
        ("retrieved", "ModelRetrieved"),
    ];

    // Scan the impl block for hook methods
    let mut listener_impls = Vec::new();
    let mut observe_calls = Vec::new();

    for item in &input.items {
        if let syn::ImplItem::Fn(method) = item {
            let method_name = method.sig.ident.to_string();
            if let Some(&(_, event_stem)) = hooks.iter().find(|(name, _)| *name == method_name) {
                let event_ident = syn::Ident::new(event_stem, method.sig.ident.span());
                let hook_method = &method.sig.ident;

                // Generate Listener<ModelEvent<M::Entity>> implementation
                listener_impls.push(quote! {
                    #[larastvel_core::async_trait]
                    impl #listener_trait<#event_mod::#event_ident<#entity_ty>> for #struct_ty {
                        async fn handle(&self, event: #event_mod::#event_ident<#entity_ty>) {
                            self.#hook_method(event.0).await;
                        }
                    }
                });

                // Generate the observe() registration call
                observe_calls.push(quote! {
                    larastvel_core::events::EventService::listen::<#event_mod::#event_ident<#entity_ty>, Self>(Self);
                });
            }
        }
    }

    let expanded = quote! {
        #input

        #(#listener_impls)*

        impl #struct_ty {
            /// Register this observer's hook methods as event listeners.
            pub fn observe() {
                #(#observe_calls)*
            }
        }
    };

    TokenStream::from(expanded)
}
