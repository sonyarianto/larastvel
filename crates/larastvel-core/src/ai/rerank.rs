//! Document reranking — the Rust equivalent of Laravel 13's
//! `Ai::rerank()` from `laravel/ai`.
//!
//! Reranking takes a query and a list of candidate documents and returns
//! them ordered by relevance to the query, typically to narrow a broader
//! first-pass retrieval.

/// Options for a rerank request.
#[derive(Debug, Clone, Default)]
pub struct RerankOptions {
    /// Override the provider's default rerank model.
    pub model: Option<String>,
}

/// The relevance of a single document, mirroring the OpenAI rerank
/// response shape.
#[derive(Debug, Clone, PartialEq)]
pub struct RerankResult {
    /// The index of the document in the request's documents array.
    pub index: usize,
    /// The relevance score (higher is more relevant).
    pub relevance_score: f64,
}

/// The result of a rerank request, mirroring the Laravel AI SDK's
/// `RerankResponse`.
#[derive(Debug, Clone, Default)]
pub struct RerankResponse {
    /// The documents ordered by relevance (most relevant first).
    pub results: Vec<RerankResult>,
}

impl RerankResponse {
    /// The best matching document index, if any.
    pub fn best(&self) -> Option<usize> {
        self.results.first().map(|result| result.index)
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rerank_response_best() {
        let response = RerankResponse {
            results: vec![
                RerankResult {
                    index: 2,
                    relevance_score: 0.9,
                },
                RerankResult {
                    index: 0,
                    relevance_score: 0.3,
                },
            ],
        };
        assert_eq!(response.best(), Some(2));
        assert!(RerankResponse::default().best().is_none());
    }
}
