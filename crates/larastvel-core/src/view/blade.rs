//! # Blade Directive Compiler
//!
//! A pre-processor that converts Laravel-style `@directive(...)` syntax into
//! Tera-equivalent syntax before the template engine renders the file.
//!
//! ## Supported Directives
//!
//! | Blade | Tera |
//! |---|---|
//! | `@auth` / `@endauth` | `{% if auth_check %}` / `{% endif %}` |
//! | `@guest` / `@endguest` | `{% if not auth_check %}` / `{% endif %}` |
//! | `@if(expr)` / `@elseif(expr)` / `@else` / `@endif` | `{% if expr %}` / `{% elif expr %}` / `{% else %}` / `{% endif %}` |
//! | `@unless(expr)` / `@endunless` | `{% if not (expr) %}` / `{% endif %}` |
//! | `@isset(var)` / `@endisset` | `{% if var is defined %}` / `{% endif %}` |
//! | `@empty(var)` / `@endempty` | `{% if var is empty %}` / `{% endif %}` |
//! | `@foreach($items as $item)` / `@endforeach` | `{% for item in items %}` / `{% endfor %}` |
//! | `@include('name')` | `{% include "name" %}` |
//! | `@error('field')` / `@enderror` | `{% if errors["field"] %}` / `{% endif %}` |
//! | `@csrf` | `<input type="hidden" ...>` |
//! | `@method('VERB')` | `<input type="hidden" ...>` |
//!
//! Variable `$` prefixes and `->` accessors are converted automatically:
//! `$user.name` → `user.name`, `$user->name` → `user.name`.
//!
//! ## Context Variables
//!
//! Templates rendered with Blade directives expect these context variables:
//!
//! * `auth_check` — `bool` indicating whether the user is authenticated
//! * `csrf_token` — `&str` containing the current CSRF token
//! * `errors` — `serde_json::Value` or `std::collections::HashMap` of field to error message(s)

use once_cell::sync::Lazy;
use regex::Regex;

/// Convert a Blade expression to a Tera expression.
///
/// Strips `$` prefixes and converts `->` to `.`:
/// - `$user` → `user`
/// - `$user->name` → `user.name`
/// - `!$user` → `not user`
fn blade_expr_to_tera(expr: &str) -> String {
    let s = expr.trim().replace("$", "").replace("->", ".");
    if s.starts_with('!') && !s[1..].trim().starts_with('=') {
        format!("not {}", s[1..].trim())
    } else {
        s
    }
}

/// Parse a `@foreach(...)` argument into a Tera `{% for ... %}` tag.
///
/// Supports:
/// - `@foreach($items as $item)` → `{% for item in items %}`
/// - `@foreach($items as $key => $value)` → `{% for key, value in items %}`
fn foreach_to_tera(args: &str) -> String {
    let args = args.trim();
    if let Some(pos) = args.find(" as ") {
        let collection = blade_expr_to_tera(&args[..pos]);
        let rest = args[pos + 4..].trim();
        if let Some(arrow) = rest.find("=>") {
            let key = blade_expr_to_tera(&rest[..arrow]);
            let value = blade_expr_to_tera(&rest[arrow + 2..]);
            format!("{{% for {key}, {value} in {collection} %}}")
        } else {
            let value = blade_expr_to_tera(rest);
            format!("{{% for {value} in {collection} %}}")
        }
    } else {
        // fallback – emit something parsable
        format!("{{% for _item in {} %}}", blade_expr_to_tera(args))
    }
}

/// Compile Blade-style `@directive` syntax into Tera-equivalent syntax.
///
/// This is a simple regex-based pre-processor. It does **not** parse
/// nested structures — it applies replacements top-to-bottom. For the
/// common directive patterns this is sufficient.
///
/// ## Example
///
/// ```rust
/// use larastvel_core::view::blade::compile;
///
/// let result = compile("@csrf");
/// assert!(result.contains("csrf_token"));
/// ```
pub fn compile(input: &str) -> String {
    let mut output = input.to_string();

    // NOTE: order matters – more specific patterns before general ones.

    // --- elseif / if / endif (must come before @else) ---
    static ELSEIF: Lazy<Regex> = Lazy::new(|| Regex::new(r"@elseif\s*\(([^)]*)\)").unwrap());
    output = ELSEIF
        .replace_all(&output, |caps: &regex::Captures| {
            format!("{{% elif {} %}}", blade_expr_to_tera(&caps[1]))
        })
        .to_string();

    static IF_START: Lazy<Regex> = Lazy::new(|| Regex::new(r"@if\s*\(([^)]*)\)").unwrap());
    output = IF_START
        .replace_all(&output, |caps: &regex::Captures| {
            format!("{{% if {} %}}", blade_expr_to_tera(&caps[1]))
        })
        .to_string();

    static ENDIF: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*@endif\s*$").unwrap());
    output = ENDIF.replace_all(&output, "{% endif %}").to_string();

    // --- unless / endunless ---
    static UNLESS_START: Lazy<Regex> = Lazy::new(|| Regex::new(r"@unless\s*\(([^)]*)\)").unwrap());
    output = UNLESS_START
        .replace_all(&output, |caps: &regex::Captures| {
            format!("{{% if not ({}) %}}", blade_expr_to_tera(&caps[1]))
        })
        .to_string();

    static ENDUNLESS: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*@endunless\s*$").unwrap());
    output = ENDUNLESS.replace_all(&output, "{% endif %}").to_string();

    // --- isset / endisset ---
    static ISS_START: Lazy<Regex> = Lazy::new(|| Regex::new(r"@isset\s*\(([^)]*)\)").unwrap());
    output = ISS_START
        .replace_all(&output, |caps: &regex::Captures| {
            format!("{{% if {} is defined %}}", blade_expr_to_tera(&caps[1]))
        })
        .to_string();

    static ENDISS: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*@endisset\s*$").unwrap());
    output = ENDISS.replace_all(&output, "{% endif %}").to_string();

    // --- empty / endempty ---
    static EMPTY_START: Lazy<Regex> = Lazy::new(|| Regex::new(r"@empty\s*\(([^)]*)\)").unwrap());
    output = EMPTY_START
        .replace_all(&output, |caps: &regex::Captures| {
            format!("{{% if {} is empty %}}", blade_expr_to_tera(&caps[1]))
        })
        .to_string();

    static ENDEMPTY: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*@endempty\s*$").unwrap());
    output = ENDEMPTY.replace_all(&output, "{% endif %}").to_string();

    // --- foreach / endforeach ---
    static FOREACH: Lazy<Regex> = Lazy::new(|| Regex::new(r"@foreach\s*\((.+?)\)").unwrap());
    output = FOREACH
        .replace_all(&output, |caps: &regex::Captures| foreach_to_tera(&caps[1]))
        .to_string();

    static ENDFOREACH: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*@endforeach\s*$").unwrap());
    output = ENDFOREACH.replace_all(&output, "{% endfor %}").to_string();

    // --- include ---
    static INCLUDE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"@include\s*\(\s*['"]([^'"]+)['"]\s*\)"#).unwrap());
    output = INCLUDE
        .replace_all(&output, |caps: &regex::Captures| {
            format!(r#"{{% include "{path}" %}}"#, path = &caps[1])
        })
        .to_string();

    // --- auth / endauth ---
    static AUTH_START: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*@auth\s*$").unwrap());
    output = AUTH_START
        .replace_all(&output, "{% if auth_check %}")
        .to_string();

    static ENDAUTH: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*@endauth\s*$").unwrap());
    output = ENDAUTH.replace_all(&output, "{% endif %}").to_string();

    // --- guest / endguest ---
    static GUEST_START: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*@guest\s*$").unwrap());
    output = GUEST_START
        .replace_all(&output, "{% if not auth_check %}")
        .to_string();

    static ENDGUEST: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*@endguest\s*$").unwrap());
    output = ENDGUEST.replace_all(&output, "{% endif %}").to_string();

    // --- error / enderror ---
    static ERROR_START: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"@error\s*\(\s*'([^']+)'\s*\)").unwrap());
    output = ERROR_START
        .replace_all(&output, |caps: &regex::Captures| {
            let field = &caps[1];
            format!(r#"{{% if errors["{field}"] %}}"#)
        })
        .to_string();

    static ENDERROR: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*@enderror\s*$").unwrap());
    output = ENDERROR.replace_all(&output, "{% endif %}").to_string();

    // --- else (generic, inside any if/auth/error block) ---
    static ELSE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*@else\s*$").unwrap());
    output = ELSE.replace_all(&output, "{% else %}").to_string();

    // --- csrf ---
    static CSRF: Lazy<Regex> = Lazy::new(|| Regex::new(r"@csrf\b").unwrap());
    output = CSRF
        .replace_all(
            &output,
            r#"<input type="hidden" name="_token" value="{{ csrf_token }}">"#,
        )
        .to_string();

    // --- method ---
    static METHOD: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"@method\s*\(\s*'([^']+)'\s*\)").unwrap());
    output = METHOD
        .replace_all(&output, |caps: &regex::Captures| {
            let method = &caps[1];
            format!(r#"<input type="hidden" name="_method" value="{}">"#, method)
        })
        .to_string();

    output
}

/// Compile a template string and inject the CSRF token value.
///
/// This is a convenience wrapper when you have the CSRF token at hand.
/// Replaces `{{ csrf_token }}` with the actual token value.
pub fn compile_with_csrf(input: &str, csrf_token: &str) -> String {
    let compiled = compile(input);
    compiled.replace("{{ csrf_token }}", csrf_token)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- @auth ---

    #[test]
    fn test_auth_directive() {
        let result = compile("@auth\n<p>Logged in</p>\n@endauth");
        assert!(result.contains("{% if auth_check %}"));
        assert!(result.contains("{% endif %}"));
        assert!(!result.contains("@auth"));
        assert!(!result.contains("@endauth"));
    }

    #[test]
    fn test_auth_with_else() {
        let result = compile("@auth\n<p>A</p>\n@else\n<p>B</p>\n@endauth");
        assert!(result.contains("{% if auth_check %}"));
        assert!(result.contains("{% else %}"));
        assert!(result.contains("{% endif %}"));
    }

    #[test]
    fn test_auth_indented() {
        let result = compile("  @auth\n    <p>Hi</p>\n  @endauth");
        assert!(result.contains("{% if auth_check %}"));
        assert!(result.contains("{% endif %}"));
    }

    // --- @guest ---

    #[test]
    fn test_guest_directive() {
        let result = compile("@guest\n<p>Guest</p>\n@endguest");
        assert!(result.contains("{% if not auth_check %}"));
        assert!(result.contains("{% endif %}"));
        assert!(!result.contains("@guest"));
    }

    // --- @error ---

    #[test]
    fn test_error_directive() {
        let result = compile("@error('email')\n<p>Invalid</p>\n@enderror");
        assert!(result.contains(r#"{% if errors["email"] %}"#));
        assert!(result.contains("{% endif %}"));
        assert!(!result.contains("@error"));
    }

    #[test]
    fn test_error_with_spaces() {
        let result = compile("@error( 'name' )\n<p>Error</p>\n@enderror");
        assert!(result.contains(r#"errors["name"]"#));
    }

    // --- @csrf ---

    #[test]
    fn test_csrf_directive() {
        let result = compile("<form>@csrf</form>");
        assert!(result.contains("csrf_token"));
        assert!(result.contains("_token"));
        assert!(result.contains("type=\"hidden\""));
    }

    #[test]
    fn test_csrf_multiple() {
        let result = compile("@csrf\n@csrf");
        assert_eq!(result.matches("csrf_token").count(), 2);
    }

    #[test]
    fn test_csrf_word_boundary() {
        // @csrf\b should not match inside @csrf_token; only match standalone @csrf
        let result = compile("@csrf_token should be preserved, but @csrf should be replaced");
        // @csrf_token should be preserved as-is (no replacement)
        assert!(result.contains("@csrf_token"));
        // The standalone @csrf should be replaced
        assert!(result.contains("<input"));
        // The replacement also emits the literal string csrf_token
        assert!(result.contains("csrf_token"));
    }

    // --- @method ---

    #[test]
    fn test_method_put() {
        let result = compile("@method('PUT')");
        assert!(result.contains("_method"));
        assert!(result.contains("PUT"));
        assert!(result.contains("type=\"hidden\""));
    }

    #[test]
    fn test_method_delete() {
        let result = compile("@method('DELETE')");
        assert!(result.contains("DELETE"));
    }

    // --- compile_with_csrf ---

    #[test]
    fn test_compile_with_csrf_replaces_token() {
        let result = compile_with_csrf("@csrf", "abc123");
        assert!(result.contains("abc123"));
        assert!(!result.contains("{{ csrf_token }}"));
    }

    // --- mixed directives ---

    #[test]
    fn test_mixed_directives() {
        let input = "\
@auth
    <form method=\"POST\">@csrf</form>
    @error('email')
        <p>{{ message }}</p>
    @enderror
@else
    @guest
        <p>Please log in</p>
    @endguest
@endauth
";
        let result = compile(input);
        assert!(result.contains("{% if auth_check %}"));
        assert!(result.contains("{% else %}"));
        assert!(result.contains("{% endif %}"));
        assert!(result.contains("csrf_token"));
        assert!(result.contains(r#"errors["email"]"#));
        assert!(result.contains("{% if not auth_check %}"));
    }

    // --- @if / @elseif / @endif ---

    #[test]
    fn test_if_basic() {
        let result = compile("@if($user)\n<p>Hi</p>\n@endif");
        assert!(result.contains("{% if user %}"));
        assert!(result.contains("{% endif %}"));
    }

    #[test]
    fn test_if_with_property() {
        let result = compile("@if($user.is_admin)\n<p>Admin</p>\n@endif");
        assert!(result.contains("{% if user.is_admin %}"));
    }

    #[test]
    fn test_if_with_arrow() {
        let result = compile("@if($user->is_admin)\n<p>Admin</p>\n@endif");
        assert!(result.contains("{% if user.is_admin %}"));
    }

    #[test]
    fn test_if_with_negation() {
        let result = compile("@if(!$user)\n<p>Guest</p>\n@endif");
        assert!(result.contains("{% if not user %}"));
    }

    #[test]
    fn test_if_elseif() {
        let result = compile("@if($role == 'admin')\n<p>A</p>\n@elseif($role == 'user')\n<p>U</p>\n@else\n<p>?</p>\n@endif");
        assert!(result.contains("{% if role == 'admin' %}"));
        assert!(result.contains("{% elif role == 'user' %}"));
        assert!(result.contains("{% else %}"));
        assert!(result.contains("{% endif %}"));
    }

    // --- @unless / @endunless ---

    #[test]
    fn test_unless() {
        let result = compile("@unless($user.is_banned)\n<p>Welcome</p>\n@endunless");
        assert!(result.contains("{% if not (user.is_banned) %}"));
        assert!(result.contains("{% endif %}"));
    }

    // --- @isset / @endisset ---

    #[test]
    fn test_isset() {
        let result = compile("@isset($name)\n<p>{{ name }}</p>\n@endisset");
        assert!(result.contains("{% if name is defined %}"));
        assert!(result.contains("{% endif %}"));
    }

    // --- @empty / @endempty ---

    #[test]
    fn test_empty() {
        let result = compile("@empty($items)\n<p>No items</p>\n@endempty");
        assert!(result.contains("{% if items is empty %}"));
        assert!(result.contains("{% endif %}"));
    }

    // --- @foreach / @endforeach ---

    #[test]
    fn test_foreach_basic() {
        let result = compile("@foreach($users as $user)\n<p>{{ user }}</p>\n@endforeach");
        assert!(result.contains("{% for user in users %}"));
        assert!(result.contains("{% endfor %}"));
    }

    #[test]
    fn test_foreach_key_value() {
        let result = compile(
            "@foreach($roles as $name => $role)\n<p>{{ name }}: {{ role }}</p>\n@endforeach",
        );
        assert!(result.contains("{% for name, role in roles %}"));
        assert!(result.contains("{% endfor %}"));
    }

    // --- @include ---

    #[test]
    fn test_include_single_quotes() {
        let result = compile("@include('partials.header')");
        assert_eq!(result.trim(), r#"{% include "partials.header" %}"#);
    }

    #[test]
    fn test_include_double_quotes() {
        let result = compile(r#"@include("partials.footer")"#);
        assert_eq!(result.trim(), r#"{% include "partials.footer" %}"#);
    }

    // --- no directives (passthrough) ---

    #[test]
    fn test_no_directives_passthrough() {
        let input = "<h1>Hello</h1>\n<p>{{ name }}</p>";
        let result = compile(input);
        assert_eq!(result, input);
    }
}
