pub fn dd(value: impl std::fmt::Debug) -> ! {
    eprintln!("{:?}", value);
    panic!("Dumped and died");
}

pub fn dump(value: impl std::fmt::Debug) {
    eprintln!("{:?}", value);
}

pub fn tap<T, F>(value: T, callback: F) -> T
where
    F: FnOnce(&T),
{
    callback(&value);
    value
}

pub fn with<T, F, R>(value: T, callback: F) -> R
where
    F: FnOnce(T) -> R,
{
    callback(value)
}

pub fn value<T: Clone>(val: &Option<T>, default: T) -> T {
    val.clone().unwrap_or(default)
}

pub fn collect<T, I: IntoIterator<Item = T>>(iter: I) -> Vec<T> {
    iter.into_iter().collect()
}

/// Builds a `302 Found` response pointing at `url` — Laravel's `redirect()`.
pub fn redirect(url: impl Into<String>) -> axum::response::Response {
    axum::response::Response::builder()
        .status(axum::http::StatusCode::FOUND)
        .header(axum::http::header::LOCATION, url.into())
        .body(axum::body::Body::empty())
        .expect("redirect response is valid")
}

/// Redirects back to the `Referer` header of `request`, falling back to `/` —
/// Laravel's `back()`.
pub fn back(req: &axum::http::Request<()>) -> axum::response::Response {
    let url = req
        .headers()
        .get(axum::http::header::REFERER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("/");
    redirect(url)
}

/// Builds an error response with the given status code and message —
/// Laravel's `abort()`.
pub fn abort(
    status: axum::http::StatusCode,
    message: impl Into<String>,
) -> axum::response::Response {
    axum::response::Response::builder()
        .status(status)
        .header(
            axum::http::header::CONTENT_TYPE,
            "text/plain; charset=utf-8",
        )
        .body(axum::body::Body::from(message.into()))
        .expect("abort response is valid")
}

/// Aborts with `status` / `message` when `condition` is true — Laravel's
/// `abort_if()`. Use with `?` inside a handler returning `Result<_, Response>`.
#[allow(clippy::result_large_err)]
pub fn abort_if(
    condition: bool,
    status: axum::http::StatusCode,
    message: impl Into<String>,
) -> Result<(), axum::response::Response> {
    if condition {
        Err(abort(status, message))
    } else {
        Ok(())
    }
}

/// Aborts with `status` / `message` unless `condition` is true — Laravel's
/// `abort_unless()`. Use with `?` inside a handler returning `Result<_, Response>`.
#[allow(clippy::result_large_err)]
pub fn abort_unless(
    condition: bool,
    status: axum::http::StatusCode,
    message: impl Into<String>,
) -> Result<(), axum::response::Response> {
    abort_if(!condition, status, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dump_does_not_panic() {
        dump("hello");
    }

    #[test]
    fn test_tap_modifies_value() {
        let mut called = false;
        let val = tap(42, |&n| {
            assert_eq!(n, 42);
            called = true;
        });
        assert_eq!(val, 42);
        assert!(called);
    }

    #[test]
    fn test_with_invokes_callback() {
        let result = with(5, |n| n * 2);
        assert_eq!(result, 10);
    }

    #[test]
    fn test_value_with_some() {
        let val = value(&Some(10), 0);
        assert_eq!(val, 10);
    }

    #[test]
    fn test_value_with_none() {
        let val: i32 = value(&None, 99);
        assert_eq!(val, 99);
    }

    #[test]
    fn test_collect_from_iterator() {
        let v = collect(1..=5);
        assert_eq!(v, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_collect_from_vec() {
        let v = collect(vec!["a", "b", "c"]);
        assert_eq!(v, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_redirect_sets_location() {
        let resp = redirect("/login");
        assert_eq!(resp.status(), axum::http::StatusCode::FOUND);
        assert_eq!(
            resp.headers()
                .get(axum::http::header::LOCATION)
                .and_then(|v| v.to_str().ok()),
            Some("/login")
        );
    }

    #[test]
    fn test_back_uses_referer() {
        let req = axum::http::Request::builder()
            .header("Referer", "/users")
            .body(())
            .unwrap();
        let resp = back(&req);
        assert_eq!(
            resp.headers()
                .get(axum::http::header::LOCATION)
                .and_then(|v| v.to_str().ok()),
            Some("/users")
        );
    }

    #[test]
    fn test_back_defaults_to_root() {
        let req = axum::http::Request::builder().body(()).unwrap();
        let resp = back(&req);
        assert_eq!(
            resp.headers()
                .get(axum::http::header::LOCATION)
                .and_then(|v| v.to_str().ok()),
            Some("/")
        );
    }

    #[test]
    fn test_abort_returns_status_and_message() {
        let resp = abort(axum::http::StatusCode::FORBIDDEN, "nope");
        assert_eq!(resp.status(), axum::http::StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_abort_if_returns_err_when_true() {
        let result: Result<(), axum::response::Response> =
            abort_if(true, axum::http::StatusCode::FORBIDDEN, "nope");
        assert!(result.is_err());
    }

    #[test]
    fn test_abort_if_returns_ok_when_false() {
        let result: Result<(), axum::response::Response> =
            abort_if(false, axum::http::StatusCode::FORBIDDEN, "nope");
        assert!(result.is_ok());
    }

    #[test]
    fn test_abort_unless_returns_ok_when_true() {
        let result: Result<(), axum::response::Response> =
            abort_unless(true, axum::http::StatusCode::FORBIDDEN, "nope");
        assert!(result.is_ok());
    }
}
