//! # Blade Directive Compiler
//!
//! A pre-processor that converts Laravel-style `@directive(...)` syntax into
//! Tera-equivalent syntax before the template engine renders the file.
//!
//! ## Supported Directives
//!
//! | Blade | Tera |
//! |---|---|
//! | `@auth` | `{% if auth_check %}` |
//! | `@else` | `{% else %}` |
//! | `@endauth` | `{% endif %}` |
//! | `@guest` | `{% if not auth_check %}` |
//! | `@endguest` | `{% endif %}` |
//! | `@error('field')` | `{% if errors["field"] %}` |
//! | `@enderror` | `{% endif %}` |
//! | `@csrf` | `<input type="hidden" ...>` |
//! | `@method('VERB')` | `<input type="hidden" ...>` |
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

    // --- else (generic, inside any auth/error block) ---
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

    // --- no directives (passthrough) ---

    #[test]
    fn test_no_directives_passthrough() {
        let input = "<h1>Hello</h1>\n<p>{{ name }}</p>";
        let result = compile(input);
        assert_eq!(result, input);
    }
}
