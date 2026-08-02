//! Content moderation — the Rust equivalent of Laravel 13's
//! `Ai::moderation()->moderate()`.

/// A moderation verdict for one category (e.g. `violence`, `harassment`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationCategory {
    /// The category id (e.g. `hate`, `self-harm`, `violence`).
    pub id: String,
    /// Whether content in this category was flagged.
    pub flagged: bool,
}

/// The result of a moderation request, mirroring the Laravel AI SDK's
/// `ModerationResponse`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationResponse {
    /// Whether the content was flagged in any category.
    pub flagged: bool,
    /// Per-category verdicts.
    pub categories: Vec<ModerationCategory>,
}

impl ModerationResponse {
    /// Whether a specific category was flagged.
    pub fn is_flagged(&self, category: &str) -> bool {
        self.categories
            .iter()
            .any(|c| c.id == category && c.flagged)
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_flagged() {
        let response = ModerationResponse {
            flagged: true,
            categories: vec![
                ModerationCategory {
                    id: "harassment".into(),
                    flagged: false,
                },
                ModerationCategory {
                    id: "violence".into(),
                    flagged: true,
                },
            ],
        };
        assert!(response.is_flagged("violence"));
        assert!(!response.is_flagged("harassment"));
        assert!(!response.is_flagged("missing"));
    }
}
