//! Image generation — the Rust equivalent of Laravel 13's `Ai::image()`
//! from `laravel/ai`.

/// Options for image requests, mirroring the Laravel AI SDK's
/// `withSize()`, `withQuality()`, `withResponseFormat()`, and `withN()`.
#[derive(Debug, Clone, Default)]
pub struct ImageOptions {
    /// The image size (e.g. `1024x1024`).
    pub size: Option<String>,
    /// The quality: `standard` or `hd`.
    pub quality: Option<String>,
    /// How the image is returned: `url` or `b64_json`.
    pub response_format: Option<String>,
    /// How many images to generate.
    pub n: Option<u32>,
}

/// A single generated image, mirroring OpenAI's image response entries.
#[derive(Debug, Clone)]
pub struct ImageResult {
    /// A remote URL for the image, when requested.
    pub url: Option<String>,
    /// Base64-encoded image data, when requested.
    pub b64_json: Option<String>,
}

impl ImageResult {
    /// The decoded image bytes, when `b64_json` was requested.
    pub fn bytes(&self) -> Option<Vec<u8>> {
        self.b64_json.as_ref().and_then(|encoded| {
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded.trim()).ok()
        })
    }
}

/// The result of an image request, mirroring the Laravel AI SDK's
/// `ImageResponse`.
#[derive(Debug, Clone)]
pub struct ImageResponse {
    /// The generated images.
    pub data: Vec<ImageResult>,
}

impl ImageResponse {
    /// The first image, if any.
    pub fn first(&self) -> Option<&ImageResult> {
        self.data.first()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_result_bytes_decodes_b64() {
        let result = ImageResult {
            url: None,
            b64_json: Some(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                [1, 2, 3],
            )),
        };
        assert_eq!(result.bytes().unwrap(), vec![1, 2, 3]);
        assert!(ImageResult {
            url: Some("https://example.test/a.png".into()),
            b64_json: None,
        }
        .bytes()
        .is_none());
    }

    #[test]
    fn test_image_response_first() {
        let response = ImageResponse {
            data: vec![ImageResult {
                url: Some("https://example.test/a.png".into()),
                b64_json: None,
            }],
        };
        assert_eq!(
            response.first().unwrap().url.as_deref(),
            Some("https://example.test/a.png")
        );
        assert!(ImageResponse { data: vec![] }.first().is_none());
    }
}
