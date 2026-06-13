use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Json, Response},
};
use serde::Serialize;
use serde_json::{json, Value};

const DEFAULT_PER_PAGE: usize = 15;
const MAX_PER_PAGE: usize = 100;

#[derive(Debug, Clone)]
pub struct Paginator<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub per_page: usize,
    pub current_page: usize,
}

impl<T> Paginator<T> {
    pub fn new(items: Vec<T>, total: usize, current_page: usize, per_page: usize) -> Self {
        Self {
            items,
            total,
            per_page: per_page.min(MAX_PER_PAGE),
            current_page,
        }
    }

    pub fn last_page(&self) -> usize {
        if self.total == 0 {
            return 1;
        }
        (self.total + self.per_page - 1) / self.per_page
    }

    pub fn from_page(items: Vec<T>, total: usize, page: &PaginationParams) -> Self {
        Self::new(items, total, page.page, page.per_page)
    }

    pub fn has_next_page(&self) -> bool {
        self.current_page < self.last_page()
    }

    pub fn has_prev_page(&self) -> bool {
        self.current_page > 1
    }

    pub fn next_page(&self) -> Option<usize> {
        if self.has_next_page() {
            Some(self.current_page + 1)
        } else {
            None
        }
    }

    pub fn prev_page(&self) -> Option<usize> {
        if self.has_prev_page() {
            Some(self.current_page - 1)
        } else {
            None
        }
    }

    pub fn from(&self) -> usize {
        ((self.current_page - 1) * self.per_page) + 1
    }

    pub fn to(&self) -> usize {
        std::cmp::min(self.current_page * self.per_page, self.total)
    }

    pub fn count(&self) -> usize {
        self.items.len()
    }

    pub fn to_json(&self) -> Value
    where
        T: Serialize,
    {
        json!({
            "data": self.items,
            "meta": {
                "current_page": self.current_page,
                "last_page": self.last_page(),
                "per_page": self.per_page,
                "total": self.total,
                "from": self.from(),
                "to": self.to(),
                "count": self.count(),
            }
        })
    }

    pub fn links(&self) -> Value {
        let base = json!({
            "first": true,
            "last": true,
            "prev": self.has_prev_page(),
            "next": self.has_next_page(),
        });
        base
    }
}

impl<T: Serialize> IntoResponse for Paginator<T> {
    fn into_response(self) -> Response {
        Json(self.to_json()).into_response()
    }
}

#[derive(Debug, Clone, Default)]
pub struct PaginationParams {
    pub page: usize,
    pub per_page: usize,
}

impl PaginationParams {
    pub fn new(page: Option<usize>, per_page: Option<usize>) -> Self {
        Self {
            page: page.unwrap_or(1).max(1),
            per_page: per_page
                .unwrap_or(DEFAULT_PER_PAGE)
                .max(1)
                .min(MAX_PER_PAGE),
        }
    }

    pub fn offset(&self) -> usize {
        (self.page - 1) * self.per_page
    }

    pub fn limit(&self) -> usize {
        self.per_page
    }
}

impl<S> FromRequestParts<S> for PaginationParams
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<Value>);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query = parts.uri.query().unwrap_or("");
        let params: std::collections::HashMap<String, String> =
            serde_urlencoded::from_str(query).unwrap_or_default();

        let page = params.get("page").and_then(|v| v.parse::<usize>().ok());
        let per_page = params
            .get("per_page")
            .or_else(|| params.get("perPage"))
            .or_else(|| params.get("per-page"))
            .and_then(|v| v.parse::<usize>().ok());

        Ok(PaginationParams::new(page, per_page))
    }
}

pub fn paginate<T>(items: Vec<T>, total: usize, params: &PaginationParams) -> Paginator<T> {
    Paginator::from_page(items, total, params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paginator_basic() {
        let items = vec![1, 2, 3, 4, 5];
        let p = Paginator::new(items.clone(), 20, 1, 5);
        assert_eq!(p.items.len(), 5);
        assert_eq!(p.total, 20);
        assert_eq!(p.current_page, 1);
        assert_eq!(p.per_page, 5);
    }

    #[test]
    fn test_last_page() {
        let p = Paginator::new(vec![1; 10], 100, 1, 10);
        assert_eq!(p.last_page(), 10);

        let p = Paginator::new(vec![1; 3], 23, 1, 10);
        assert_eq!(p.last_page(), 3);

        let p = Paginator::<i32>::new(vec![], 0, 1, 10);
        assert_eq!(p.last_page(), 1);
    }

    #[test]
    fn test_has_next_prev_page() {
        let p = Paginator::new(vec![1; 10], 50, 1, 10);
        assert!(p.has_next_page());
        assert!(!p.has_prev_page());

        let p = Paginator::new(vec![1; 10], 50, 5, 10);
        assert!(!p.has_next_page());
        assert!(p.has_prev_page());

        let p = Paginator::new(vec![1; 10], 50, 3, 10);
        assert!(p.has_next_page());
        assert!(p.has_prev_page());
    }

    #[test]
    fn test_next_prev_page() {
        let p = Paginator::new(vec![1; 10], 50, 1, 10);
        assert_eq!(p.next_page(), Some(2));
        assert_eq!(p.prev_page(), None);

        let p = Paginator::new(vec![1; 10], 50, 5, 10);
        assert_eq!(p.next_page(), None);
        assert_eq!(p.prev_page(), Some(4));
    }

    #[test]
    fn test_from_to() {
        let p = Paginator::new(vec![1; 5], 20, 2, 5);
        assert_eq!(p.from(), 6);
        assert_eq!(p.to(), 10);

        let p = Paginator::new(vec![1; 3], 13, 3, 5);
        assert_eq!(p.from(), 11);
        assert_eq!(p.to(), 13);
    }

    #[test]
    fn test_to_json_structure() {
        let items = vec!["a", "b", "c"];
        let p = Paginator::new(items, 12, 1, 3);
        let json = p.to_json();

        assert!(json["data"].is_array());
        assert_eq!(json["data"].as_array().unwrap().len(), 3);
        assert_eq!(json["meta"]["current_page"], 1);
        assert_eq!(json["meta"]["last_page"], 4);
        assert_eq!(json["meta"]["per_page"], 3);
        assert_eq!(json["meta"]["total"], 12);
        assert_eq!(json["meta"]["from"], 1);
        assert_eq!(json["meta"]["to"], 3);
        assert_eq!(json["meta"]["count"], 3);
    }

    #[test]
    fn test_pagination_params_default() {
        let params = PaginationParams::new(None, None);
        assert_eq!(params.page, 1);
        assert_eq!(params.per_page, 15);
    }

    #[test]
    fn test_pagination_params_custom() {
        let params = PaginationParams::new(Some(3), Some(25));
        assert_eq!(params.page, 3);
        assert_eq!(params.per_page, 25);
    }

    #[test]
    fn test_pagination_params_clamp() {
        let params = PaginationParams::new(Some(0), Some(0));
        assert_eq!(params.page, 1);
        assert_eq!(params.per_page, 1);

        let params = PaginationParams::new(Some(999), Some(999));
        assert_eq!(params.per_page, 100);
    }

    #[test]
    fn test_pagination_params_offset() {
        let params = PaginationParams::new(Some(3), Some(10));
        assert_eq!(params.offset(), 20);
    }

    #[test]
    fn test_from_page() {
        let params = PaginationParams::new(Some(2), Some(5));
        let p = Paginator::from_page(vec![1, 2, 3], 23, &params);
        assert_eq!(p.current_page, 2);
        assert_eq!(p.per_page, 5);
        assert_eq!(p.total, 23);
    }

    #[test]
    fn test_paginate_function() {
        let params = PaginationParams::new(Some(1), Some(10));
        let items = vec!["x"; 10];
        let p = paginate(items, 50, &params);
        assert_eq!(p.current_page, 1);
        assert_eq!(p.per_page, 10);
        assert_eq!(p.last_page(), 5);
    }

    #[test]
    fn test_max_per_page_clamped() {
        let p = Paginator::new(vec![1; 200], 1000, 1, 200);
        assert_eq!(p.per_page, 100);
    }
}
