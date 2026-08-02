//! Content-bearing media values — the Rust equivalent of Laravel 13's
//! `Media` value object from `laravel/ai`.
//!
//! `Media` wraps raw bytes with a MIME type, supporting images, audio,
//! text, and arbitrary files, and is the input type for image editing,
//! image variations, and speech-to-text.

/// A piece of media content — bytes plus a MIME type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Media {
    content: Vec<u8>,
    mime_type: String,
}

impl Media {
    /// Create media from text content (UTF-8).
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into().into_bytes(),
            mime_type: "text/plain".into(),
        }
    }

    /// Create audio media from raw bytes and a MIME type.
    pub fn audio(content: impl Into<Vec<u8>>, mime_type: impl Into<String>) -> Self {
        Self::file(content, mime_type)
    }

    /// Create image media from raw bytes and a MIME type.
    pub fn image(content: impl Into<Vec<u8>>, mime_type: impl Into<String>) -> Self {
        Self::file(content, mime_type)
    }

    /// Create media from raw bytes and a MIME type — Laravel's
    /// `Media::file()`.
    pub fn file(content: impl Into<Vec<u8>>, mime_type: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            mime_type: mime_type.into(),
        }
    }

    /// Create media from a base64-encoded payload and a MIME type —
    /// Laravel's `Media::fromBase64()`.
    pub fn from_base64(encoded: &str, mime_type: impl Into<String>) -> Self {
        let content =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded.trim())
                .unwrap_or_default();
        Self {
            content,
            mime_type: mime_type.into(),
        }
    }

    /// The raw bytes.
    pub fn content(&self) -> &[u8] {
        &self.content
    }

    /// The MIME type (e.g. `image/png`, `audio/mpeg`).
    pub fn mime_type(&self) -> &str {
        &self.mime_type
    }

    /// The content encoded as base64.
    pub fn base64(&self) -> String {
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &self.content)
    }

    /// The content interpreted as text, when it is textual.
    pub fn text_content(&self) -> Option<String> {
        String::from_utf8(self.content.clone()).ok()
    }
}

/// Options for speech requests (TTS and STT), mirroring the Laravel AI
/// SDK's `withVoice()`, `withFormat()`, `withLanguage()`, and
/// `withSpeed()`.
#[derive(Debug, Clone, Default)]
pub struct AudioOptions {
    /// The voice to synthesize with (e.g. `alloy`, `echo`, `shimmer`).
    pub voice: Option<String>,
    /// The audio format (e.g. `mp3`, `opus`, `wav`).
    pub format: Option<String>,
    /// The input language (e.g. `en`, `id`).
    pub language: Option<String>,
    /// The speech speed (0.25–4.0, default 1.0).
    pub speed: Option<f32>,
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_text() {
        let media = Media::text("hello");
        assert_eq!(media.content(), b"hello");
        assert_eq!(media.mime_type(), "text/plain");
        assert_eq!(media.text_content().as_deref(), Some("hello"));
    }

    #[test]
    fn test_media_base64_round_trip() {
        let media = Media::image(vec![1, 2, 3, 4], "image/png");
        let encoded = media.base64();
        assert_eq!(encoded, "AQIDBA==");
        let decoded = Media::from_base64(&encoded, "image/png");
        assert_eq!(decoded.content(), &[1, 2, 3, 4]);
        assert_eq!(decoded.mime_type(), "image/png");
    }

    #[test]
    fn test_media_file_constructors() {
        let audio = Media::audio(vec![0x00, 0xff], "audio/mpeg");
        assert_eq!(audio.mime_type(), "audio/mpeg");
        let image = Media::image(vec![0x89], "image/png");
        assert_eq!(image.mime_type(), "image/png");
    }

    #[test]
    fn test_media_from_base64_invalid_returns_empty() {
        let media = Media::from_base64("!!!not-base64!!!", "image/png");
        assert!(media.content().is_empty());
    }
}
