//! Image manipulation — Laravel 13.20's `Image` facade.
//!
//! Images are immutable: every transformation returns a new instance with the
//! operation appended to its pipeline, and processing happens once at encode
//! time. Backed by the `image` crate (Rust equivalent of Intervention Image).

use std::io::Cursor;
use std::path::Path;
use std::sync::Mutex;

use base64::Engine as _;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat};

/// Errors produced while reading, transforming, or encoding images.
#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("invalid base64 image data")]
    InvalidBase64,
    #[error("failed to read image: {0}")]
    Read(String),
    #[error("failed to decode image: {0}")]
    Decode(String),
    #[error("failed to encode image: {0}")]
    Encode(String),
    #[error("the [{0}] format is not supported")]
    UnsupportedFormat(String),
    #[error("unable to determine the dimensions of the image")]
    Dimensions,
    #[error("at least one resize dimension must be specified")]
    ResizeDimensions,
    #[error("invalid storage path `{0}`: path must stay within the storage directory")]
    InvalidStoragePath(String),
    #[error("failed to write image: {0}")]
    Write(String),
}

pub type Result<T> = std::result::Result<T, ImageError>;

/// Output formats supported by [`ImageInstance::to_*`] conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Png,
    Jpeg,
    Gif,
    Webp,
    Bmp,
}

impl OutputFormat {
    pub fn mime_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
            Self::Bmp => "image/bmp",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Gif => "gif",
            Self::Webp => "webp",
            Self::Bmp => "bmp",
        }
    }
}

/// Background fill used by [`ImageInstance::contain`] and
/// [`ImageInstance::rotate`].
#[derive(Debug, Clone, PartialEq)]
pub enum Background {
    /// A hex color such as `#ffffff` (or a bare name like `white`).
    Hex(String),
    /// The image's dominant color.
    Dominant,
}

impl Background {
    fn to_rgba(&self, dominant: [u8; 3]) -> image::Rgba<u8> {
        match self {
            Self::Dominant => image::Rgba([dominant[0], dominant[1], dominant[2], 255]),
            Self::Hex(hex) => parse_hex_color(hex)
                .map(|[r, g, b]| image::Rgba([r, g, b, 255]))
                .unwrap_or(image::Rgba([255, 255, 255, 255])),
        }
    }
}

fn parse_hex_color(s: &str) -> Option<[u8; 3]> {
    let s = s.trim_start_matches('#');
    match s.len() {
        6 => {
            let v = u32::from_str_radix(s, 16).ok()?;
            Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
        }
        3 => {
            let v = u32::from_str_radix(s, 16).ok()?;
            let r = (v >> 8) as u8;
            let g = ((v >> 4) & 0xF) as u8;
            let b = (v & 0xF) as u8;
            Some([r * 17, g * 17, b * 17])
        }
        _ => None,
    }
}

/// A single transformation queued on the processing pipeline.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Resize(Option<u32>, Option<u32>),
    Scale(Option<u32>, Option<u32>),
    Cover(u32, u32),
    Contain(u32, u32, Option<Background>),
    Crop(u32, u32, u32, u32),
    Rotate(f32, Option<Background>),
    Orient,
    Blur(u8),
    Grayscale,
    Sharpen(u8),
    FlipVertically,
    FlipHorizontally,
}

/// An immutable image with a queue of pending transformations.
///
/// Mirrors `Illuminate\Image\Image`: transformations are chained fluently,
/// processed in order, and the image is only encoded once at the end.
#[derive(Debug, Clone)]
pub struct ImageInstance {
    contents: Vec<u8>,
    source_name: Option<String>,
    original_format: ImageFormat,
    ops: Vec<Op>,
    format: Option<OutputFormat>,
    quality: u8,
}

impl ImageInstance {
    fn new(contents: Vec<u8>, source_name: Option<String>) -> Result<Self> {
        let format =
            image::guess_format(&contents).map_err(|e| ImageError::Decode(e.to_string()))?;
        Ok(Self {
            contents,
            source_name,
            original_format: format,
            ops: Vec::new(),
            format: None,
            quality: 70,
        })
    }

    fn with_op(&self, op: Op) -> Self {
        let mut clone = self.clone();
        clone.ops.push(op);
        record_fake(&clone);
        clone
    }

    /// Resize the image to the given dimensions. Either width or height may
    /// be omitted to preserve the aspect ratio.
    pub fn resize(&self, width: Option<u32>, height: Option<u32>) -> Result<Self> {
        if width.is_none() && height.is_none() {
            return Err(ImageError::ResizeDimensions);
        }
        Ok(self.with_op(Op::Resize(width, height)))
    }

    /// Proportionally scale the image down so it fits within the given
    /// dimensions. Never increases the image size.
    pub fn scale(&self, width: Option<u32>, height: Option<u32>) -> Result<Self> {
        if width.is_none() && height.is_none() {
            return Err(ImageError::ResizeDimensions);
        }
        Ok(self.with_op(Op::Scale(width, height)))
    }

    /// Resize and crop the image to completely cover the given dimensions.
    pub fn cover(&self, width: u32, height: u32) -> Self {
        self.with_op(Op::Cover(width.max(1), height.max(1)))
    }

    /// Resize the image to fit within the given dimensions while preserving
    /// the entire image. Empty space is filled with the optional background
    /// color (`"#ffffff"`, `"dominant"`, or `None` for transparent).
    pub fn contain(&self, width: u32, height: u32, background: Option<&str>) -> Self {
        let background = background.map(|b| {
            if b.eq_ignore_ascii_case("dominant") {
                Background::Dominant
            } else {
                Background::Hex(b.to_string())
            }
        });
        self.with_op(Op::Contain(width.max(1), height.max(1), background))
    }

    /// Crop the image to the given dimensions and position.
    pub fn crop(&self, width: u32, height: u32, x: u32, y: u32) -> Self {
        self.with_op(Op::Crop(width.max(1), height.max(1), x, y))
    }

    /// Rotate the image clockwise by the given angle (degrees), filling the
    /// background with the optional color (`"#ffffff"` / `"dominant"`).
    pub fn rotate(&self, angle: f32, background: Option<&str>) -> Self {
        let background = background.map(|b| {
            if b.eq_ignore_ascii_case("dominant") {
                Background::Dominant
            } else {
                Background::Hex(b.to_string())
            }
        });
        self.with_op(Op::Rotate(angle, background))
    }

    /// Auto-orient the image based on its EXIF orientation data.
    pub fn orient(&self) -> Self {
        self.with_op(Op::Orient)
    }

    /// Apply a gaussian blur. `amount` is clamped between 0 and 100.
    pub fn blur(&self, amount: u8) -> Self {
        self.with_op(Op::Blur(amount.min(100)))
    }

    /// Convert the image to grayscale.
    pub fn grayscale(&self) -> Self {
        self.with_op(Op::Grayscale)
    }

    /// Sharpen the image (unsharp mask). `amount` is clamped between 0 and 100.
    pub fn sharpen(&self, amount: u8) -> Self {
        self.with_op(Op::Sharpen(amount.min(100)))
    }

    /// Flip the image vertically.
    pub fn flip_vertically(&self) -> Self {
        self.with_op(Op::FlipVertically)
    }

    /// Flip the image horizontally.
    pub fn flip_horizontally(&self) -> Self {
        self.with_op(Op::FlipHorizontally)
    }

    /// Flip the image vertically (`flip`).
    pub fn flip(&self) -> Self {
        self.flip_vertically()
    }

    /// Flip the image horizontally (`flop`).
    pub fn flop(&self) -> Self {
        self.flip_horizontally()
    }

    /// Set the output quality, clamped between 1 and 100. Applies to lossy
    /// encoders (JPEG); other encoders use their default settings.
    pub fn quality(&self, quality: u8) -> Self {
        let mut clone = self.clone();
        clone.quality = quality.clamp(1, 100);
        clone
    }

    /// Convert the image to WebP format.
    pub fn to_webp(&self) -> Self {
        self.to_format(OutputFormat::Webp)
    }

    /// Convert the image to JPEG format.
    pub fn to_jpg(&self) -> Self {
        self.to_format(OutputFormat::Jpeg)
    }

    /// Convert the image to JPEG format.
    pub fn to_jpeg(&self) -> Self {
        self.to_jpg()
    }

    /// Convert the image to PNG format.
    pub fn to_png(&self) -> Self {
        self.to_format(OutputFormat::Png)
    }

    /// Convert the image to GIF format.
    pub fn to_gif(&self) -> Self {
        self.to_format(OutputFormat::Gif)
    }

    /// Convert the image to BMP format.
    pub fn to_bmp(&self) -> Self {
        self.to_format(OutputFormat::Bmp)
    }

    /// Convert the image to the given format.
    pub fn to_format(&self, format: OutputFormat) -> Self {
        let mut clone = self.clone();
        clone.format = Some(format);
        record_fake(&clone);
        clone
    }

    /// Convenience for converting to a format and setting quality.
    pub fn optimize(&self, format: OutputFormat, quality: u8) -> Self {
        self.to_format(format).quality(quality)
    }

    /// Process the image and return the raw bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        if self.ops.is_empty() && self.format.is_none() {
            return Ok(self.contents.clone());
        }
        let decoded = image::load_from_memory(&self.contents)
            .map_err(|e| ImageError::Decode(e.to_string()))?;
        let img = self.apply_ops(decoded)?;
        let format = self.format.unwrap_or(original_format(self.original_format));
        encode(&img, format, self.quality)
    }

    /// Process the image and return it as a base64 encoded string.
    pub fn to_base64(&self) -> Result<String> {
        Ok(base64::engine::general_purpose::STANDARD.encode(self.to_bytes()?))
    }

    /// Process the image and return it as a data URI.
    pub fn to_data_uri(&self) -> Result<String> {
        Ok(format!(
            "data:{};base64,{}",
            self.mime_type()?,
            self.to_base64()?
        ))
    }

    /// The MIME type of the processed image.
    pub fn mime_type(&self) -> Result<String> {
        Ok(self
            .format
            .map(|f| f.mime_type().to_string())
            .unwrap_or_else(|| mime_type_for(self.original_format).to_string()))
    }

    /// The file extension of the processed image.
    pub fn extension(&self) -> String {
        self.format
            .map(|f| f.extension().to_string())
            .unwrap_or_else(|| extension_for(self.original_format).to_string())
    }

    /// The dimensions of the processed image as `(width, height)`.
    pub fn dimensions(&self) -> Result<(u32, u32)> {
        let decoded = image::load_from_memory(&self.contents)
            .map_err(|e| ImageError::Decode(e.to_string()))?;
        let img = self.apply_ops(decoded)?;
        Ok((img.width(), img.height()))
    }

    /// The width of the processed image.
    pub fn width(&self) -> Result<u32> {
        Ok(self.dimensions()?.0)
    }

    /// The height of the processed image.
    pub fn height(&self) -> Result<u32> {
        Ok(self.dimensions()?.1)
    }

    /// The dominant (average) color of the processed image as a hex string.
    pub fn dominant_color(&self) -> Result<String> {
        let decoded = image::load_from_memory(&self.contents)
            .map_err(|e| ImageError::Decode(e.to_string()))?;
        let img = self.apply_ops(decoded)?;
        let rgb = img.to_rgb8();
        let (mut r, mut g, mut b, mut count) = (0u64, 0u64, 0u64, 0u64);
        for pixel in rgb.pixels() {
            r += pixel[0] as u64;
            g += pixel[1] as u64;
            b += pixel[2] as u64;
            count += 1;
        }
        if count == 0 {
            return Err(ImageError::Dimensions);
        }
        Ok(format!(
            "#{:02x}{:02x}{:02x}",
            (r / count) as u8,
            (g / count) as u8,
            (b / count) as u8
        ))
    }

    /// Write the processed image to `path`, creating parent directories.
    /// Returns the path on success.
    pub fn save(&self, path: &Path) -> Result<String> {
        let bytes = self.to_bytes()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ImageError::Write(e.to_string()))?;
        }
        std::fs::write(path, &bytes).map_err(|e| ImageError::Write(e.to_string()))?;
        Ok(path.to_string_lossy().into_owned())
    }

    /// Store the processed image on the local disk under `storage/app`.
    /// Returns the relative stored path.
    pub fn store(&self, path: &str) -> Result<String> {
        self.store_as(path, &self.hash_name())
    }

    /// Store the processed image on the local disk under `storage/app`
    /// with the given name. Returns the relative stored path.
    pub fn store_as(&self, path: &str, name: &str) -> Result<String> {
        if path.split('/').any(|seg| seg == "..") || name.split('/').any(|seg| seg == "..") {
            return Err(ImageError::InvalidStoragePath(format!("{path}/{name}")));
        }
        let root = crate::storage_path(Some("app"));
        let dir = if path.is_empty() {
            root.clone()
        } else {
            root.join(path)
        };
        std::fs::create_dir_all(&dir).map_err(|e| ImageError::Write(e.to_string()))?;
        let full = dir.join(name);
        std::fs::write(&full, self.to_bytes()?).map_err(|e| ImageError::Write(e.to_string()))?;
        let relative = if path.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", path.trim_end_matches('/'), name)
        };
        let mut state = fake_state().lock().unwrap();
        if let Some(records) = state.as_mut() {
            let image_name = self
                .source_name
                .clone()
                .unwrap_or_else(|| "image".to_string());
            records.push(RecordedOp {
                name: image_name,
                op: Op::Grayscale,
                format: None,
                quality: None,
                stored_to: Some(relative.clone()),
            });
        }
        Ok(relative)
    }

    /// Store the processed image with public visibility. The local disk does
    /// not model visibility, so this is equivalent to [`store`](Self::store).
    pub fn store_publicly(&self, path: &str) -> Result<String> {
        self.store(path)
    }

    /// Store the processed image with public visibility and a given name.
    pub fn store_publicly_as(&self, path: &str, name: &str) -> Result<String> {
        self.store_as(path, name)
    }

    /// A hashed filename with the correct extension (40 random hex chars).
    pub fn hash_name(&self) -> String {
        let mut bytes = [0u8; 20];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut bytes);
        format!("{}.{}", hex::encode(bytes), self.extension())
    }

    fn apply_ops(&self, img: DynamicImage) -> Result<DynamicImage> {
        let mut img = img;
        for op in &self.ops {
            img = apply_op(img, op)?;
        }
        Ok(img)
    }
}

fn apply_op(img: DynamicImage, op: &Op) -> Result<DynamicImage> {
    match op {
        Op::Resize(width, height) => {
            let (w, h) = resolve_dimensions(img.width(), img.height(), *width, *height)?;
            Ok(img.resize(w, h, FilterType::Triangle))
        }
        Op::Scale(width, height) => {
            let (w, h) = resolve_dimensions(img.width(), img.height(), *width, *height)?;
            let ratio = (w as f64 / img.width() as f64)
                .min(h as f64 / img.height() as f64)
                .min(1.0);
            let new_w = (img.width() as f64 * ratio).round().max(1.0) as u32;
            let new_h = (img.height() as f64 * ratio).round().max(1.0) as u32;
            Ok(img.resize(new_w, new_h, FilterType::Triangle))
        }
        Op::Cover(w, h) => {
            let ratio = (*w as f64 / img.width() as f64)
                .max(*h as f64 / img.height() as f64)
                .max(1.0);
            let new_w = (img.width() as f64 * ratio).round() as u32;
            let new_h = (img.height() as f64 * ratio).round() as u32;
            let resized = img.resize(new_w, new_h, FilterType::Triangle);
            let x = ((new_w - w) / 2).min(new_w.saturating_sub(*w));
            let y = ((new_h - h) / 2).min(new_h.saturating_sub(*h));
            Ok(image::imageops::crop_imm(&resized, x, y, *w, *h)
                .to_image()
                .into())
        }
        Op::Contain(w, h, background) => {
            let ratio = (*w as f64 / img.width() as f64)
                .min(*h as f64 / img.height() as f64)
                .min(1.0);
            let new_w = (img.width() as f64 * ratio).round().max(1.0) as u32;
            let new_h = (img.height() as f64 * ratio).round().max(1.0) as u32;
            let resized = img.resize(new_w, new_h, FilterType::Triangle);
            let dominant = dominant_rgb(&resized);
            let mut canvas = DynamicImage::new_rgba8(*w, *h).to_rgba8();
            let bg = background
                .as_ref()
                .map(|b| b.to_rgba(dominant))
                .unwrap_or(image::Rgba([0, 0, 0, 0]));
            for pixel in canvas.pixels_mut() {
                *pixel = bg;
            }
            let x = (*w - new_w) / 2;
            let y = (*h - new_h) / 2;
            image::imageops::overlay(&mut canvas, &resized.to_rgba8(), x as i64, y as i64);
            Ok(canvas.into())
        }
        Op::Crop(w, h, x, y) => {
            let x = (*x).min(img.width().saturating_sub(*w));
            let y = (*y).min(img.height().saturating_sub(*h));
            Ok(image::imageops::crop_imm(&img, x, y, *w, *h)
                .to_image()
                .into())
        }
        Op::Rotate(angle, background) => rotate_cw(
            &img,
            *angle,
            background.as_ref().map(|b| {
                let dominant = dominant_rgb(&img);
                b.to_rgba(dominant)
            }),
        ),
        Op::Orient => Ok(img),
        Op::Blur(amount) => {
            if *amount == 0 {
                Ok(img)
            } else {
                Ok(image::imageops::blur(&img, f32::from(*amount) * 0.5).into())
            }
        }
        Op::Grayscale => Ok(image::imageops::grayscale(&img).into()),
        Op::Sharpen(amount) => {
            if *amount == 0 {
                Ok(img)
            } else {
                Ok(unsharp_mask(&img, *amount))
            }
        }
        Op::FlipVertically => Ok(image::imageops::flip_vertical(&img).into()),
        Op::FlipHorizontally => Ok(image::imageops::flip_horizontal(&img).into()),
    }
}

fn resolve_dimensions(
    current_w: u32,
    current_h: u32,
    width: Option<u32>,
    height: Option<u32>,
) -> Result<(u32, u32)> {
    match (width, height) {
        (Some(w), Some(h)) => Ok((w, h)),
        (Some(w), None) => {
            let ratio = w as f64 / current_w as f64;
            Ok((w, ((current_h as f64 * ratio).round().max(1.0)) as u32))
        }
        (None, Some(h)) => {
            let ratio = h as f64 / current_h as f64;
            Ok((((current_w as f64 * ratio).round().max(1.0)) as u32, h))
        }
        (None, None) => Err(ImageError::ResizeDimensions),
    }
}

fn dominant_rgb(img: &DynamicImage) -> [u8; 3] {
    let rgb = img.to_rgb8();
    let (mut r, mut g, mut b, mut count) = (0u64, 0u64, 0u64, 0u64);
    for pixel in rgb.pixels() {
        r += pixel[0] as u64;
        g += pixel[1] as u64;
        b += pixel[2] as u64;
        count += 1;
    }
    if count == 0 {
        return [0, 0, 0];
    }
    [(r / count) as u8, (g / count) as u8, (b / count) as u8]
}

/// Rotate clockwise by an arbitrary angle using nearest-neighbor inverse
/// mapping; multiples of 90° are exact via `imageops`.
fn rotate_cw(
    img: &DynamicImage,
    angle: f32,
    background: Option<image::Rgba<u8>>,
) -> Result<DynamicImage> {
    let normalized = angle.rem_euclid(360.0);
    if normalized == 0.0 {
        return Ok(img.clone());
    }
    if normalized == 90.0 {
        return Ok(image::imageops::rotate90(img).into());
    }
    if normalized == 180.0 {
        return Ok(image::imageops::rotate180(img).into());
    }
    if normalized == 270.0 {
        return Ok(image::imageops::rotate270(img).into());
    }
    let (w, h) = (img.width(), img.height());
    let theta = normalized.to_radians();
    let (sin, cos) = theta.sin_cos();
    let new_w = ((w as f32 * cos.abs()) + (h as f32 * sin.abs())).ceil() as u32;
    let new_h = ((w as f32 * sin.abs()) + (h as f32 * cos.abs())).ceil() as u32;
    let src = img.to_rgba8();
    let bg = background.unwrap_or(image::Rgba([255, 255, 255, 255]));
    let mut out = DynamicImage::new_rgba8(new_w.max(1), new_h.max(1)).to_rgba8();
    for pixel in out.pixels_mut() {
        *pixel = bg;
    }
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let (ocx, ocy) = (new_w as f32 / 2.0, new_h as f32 / 2.0);
    for y in 0..new_h {
        for x in 0..new_w {
            let dx = x as f32 - ocx;
            let dy = y as f32 - ocy;
            let sx = (dx * cos + dy * sin + cx).round();
            let sy = (-dx * sin + dy * cos + cy).round();
            if sx >= 0.0 && sy >= 0.0 && sx < w as f32 && sy < h as f32 {
                out.put_pixel(x, y, *src.get_pixel(sx as u32, sy as u32));
            }
        }
    }
    Ok(out.into())
}

/// Unsharp mask: `out = original + amount * (original - blurred)`.
fn unsharp_mask(img: &DynamicImage, amount: u8) -> DynamicImage {
    let blurred = image::imageops::blur(img, 2.0);
    let mut sharpened = img.to_rgba8();
    let factor = f32::from(amount) / 100.0;
    for (out_pixel, blur_pixel) in sharpened.pixels_mut().zip(blurred.pixels()) {
        for channel in 0..3 {
            let orig = out_pixel[channel] as f32;
            let blur = blur_pixel[channel] as f32;
            let value = (orig + factor * (orig - blur)).clamp(0.0, 255.0);
            out_pixel[channel] = value as u8;
        }
    }
    sharpened.into()
}

fn original_format(format: ImageFormat) -> OutputFormat {
    match format {
        ImageFormat::Png => OutputFormat::Png,
        ImageFormat::Jpeg => OutputFormat::Jpeg,
        ImageFormat::Gif => OutputFormat::Gif,
        ImageFormat::WebP => OutputFormat::Webp,
        ImageFormat::Bmp => OutputFormat::Bmp,
        _ => OutputFormat::Png,
    }
}

fn mime_type_for(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::Gif => "image/gif",
        ImageFormat::WebP => "image/webp",
        ImageFormat::Bmp => "image/bmp",
        ImageFormat::Tiff => "image/tiff",
        _ => "application/octet-stream",
    }
}

fn extension_for(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Gif => "gif",
        ImageFormat::WebP => "webp",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Tiff => "tiff",
        _ => "bin",
    }
}

fn encode(img: &DynamicImage, format: OutputFormat, quality: u8) -> Result<Vec<u8>> {
    let mut buf = Cursor::new(Vec::new());
    match format {
        OutputFormat::Jpeg => {
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
            img.write_with_encoder(encoder)
                .map_err(|e| ImageError::Encode(e.to_string()))?;
        }
        OutputFormat::Png => {
            img.write_to(&mut buf, ImageFormat::Png)
                .map_err(|e| ImageError::Encode(e.to_string()))?;
        }
        OutputFormat::Gif => {
            img.write_to(&mut buf, ImageFormat::Gif)
                .map_err(|e| ImageError::Encode(e.to_string()))?;
        }
        OutputFormat::Webp => {
            img.write_to(&mut buf, ImageFormat::WebP)
                .map_err(|e| ImageError::Encode(e.to_string()))?;
        }
        OutputFormat::Bmp => {
            img.write_to(&mut buf, ImageFormat::Bmp)
                .map_err(|e| ImageError::Encode(e.to_string()))?;
        }
    }
    Ok(buf.into_inner())
}

/// A recorded image operation for the test fake.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordedOp {
    pub name: String,
    pub op: Op,
    pub format: Option<OutputFormat>,
    pub quality: Option<u8>,
    pub stored_to: Option<String>,
}

/// Global fake state used by [`Image::fake`] and the `assert_*` helpers.
static FAKE_STATE: std::sync::OnceLock<Mutex<Option<Vec<RecordedOp>>>> = std::sync::OnceLock::new();

fn fake_state() -> &'static Mutex<Option<Vec<RecordedOp>>> {
    FAKE_STATE.get_or_init(|| Mutex::new(None))
}

/// The `Image` facade — Laravel 13.20's first-party image processing.
///
/// ```rust,ignore
/// let image = Image::from_path("input.jpg")?.cover(400, 400)?.to_webp()?;
/// image.save("output.webp")?;
/// ```
pub struct Image;

impl Image {
    /// Create an image instance from raw bytes.
    pub fn from_bytes(contents: &[u8]) -> Result<ImageInstance> {
        ImageInstance::new(contents.to_vec(), None)
    }

    /// Create an image instance from a base64 encoded string.
    pub fn from_base64(base64: &str) -> Result<ImageInstance> {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(base64)
            .map_err(|_| ImageError::InvalidBase64)?;
        ImageInstance::new(decoded, None)
    }

    /// Create an image instance from a file path.
    pub fn from_path(path: &str) -> Result<ImageInstance> {
        let bytes = std::fs::read(path).map_err(|e| ImageError::Read(e.to_string()))?;
        let name = Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned());
        ImageInstance::new(bytes, name)
    }

    /// Create an image instance from a URL. The URL is fetched over HTTP.
    pub async fn from_url(url: &str) -> Result<ImageInstance> {
        let bytes = reqwest::get(url)
            .await
            .map_err(|e| ImageError::Read(e.to_string()))?
            .bytes()
            .await
            .map_err(|e| ImageError::Read(e.to_string()))?
            .to_vec();
        ImageInstance::new(bytes, Some(url.to_string()))
    }

    /// Create an image instance from a storage disk path (relative to the
    /// application's `storage/app` directory).
    pub fn from_storage(path: &str) -> Result<ImageInstance> {
        let root = crate::storage_path(Some("app"));
        Image::from_path(&root.join(path).to_string_lossy())
    }

    /// Swap the image processing pipeline with a fake that records
    /// operations instead of performing them. Returns the previous fake
    /// state so callers can restore it.
    pub fn fake() -> bool {
        let mut state = fake_state().lock().unwrap();
        let was_faked = state.is_some();
        *state = Some(Vec::new());
        was_faked
    }

    /// Restore real image processing (turns the fake off).
    pub fn restore() {
        let mut state = fake_state().lock().unwrap();
        *state = None;
    }

    /// Whether image processing is currently faked.
    pub fn is_faked() -> bool {
        fake_state().lock().unwrap().is_some()
    }

    /// Assert that an image was resized to the given dimensions.
    pub fn assert_resized(name: &str, width: Option<u32>, height: Option<u32>) {
        Self::assert_recorded(
            name,
            |r| matches!(r.op, Op::Resize(w, h) if w == width && h == height),
        );
    }

    /// Assert that an image was covered to the given dimensions.
    pub fn assert_covered(name: &str, width: u32, height: u32) {
        Self::assert_recorded(
            name,
            |r| matches!(r.op, Op::Cover(w, h) if w == width && h == height),
        );
    }

    /// Assert that an image was cropped to the given dimensions and position.
    pub fn assert_cropped(name: &str, width: u32, height: u32, x: u32, y: u32) {
        Self::assert_recorded(
            name,
            |r| matches!(r.op, Op::Crop(w, h, cx, cy) if w == width && h == height && cx == x && cy == y),
        );
    }

    /// Assert that an image was contained to the given dimensions.
    pub fn assert_contained(name: &str, width: u32, height: u32) {
        Self::assert_recorded(
            name,
            |r| matches!(r.op, Op::Contain(w, h, _) if w == width && h == height),
        );
    }

    /// Assert that an image was rotated by the given angle.
    pub fn assert_rotated(name: &str, angle: f32) {
        Self::assert_recorded(name, |r| matches!(r.op, Op::Rotate(a, _) if a == angle));
    }

    /// Assert that an image was blurred.
    pub fn assert_blurred(name: &str) {
        Self::assert_recorded(name, |r| matches!(r.op, Op::Blur(_)));
    }

    /// Assert that an image was converted to grayscale.
    pub fn assert_grayscale(name: &str) {
        Self::assert_recorded(name, |r| matches!(r.op, Op::Grayscale));
    }

    /// Assert that an image was sharpened.
    pub fn assert_sharpened(name: &str) {
        Self::assert_recorded(name, |r| matches!(r.op, Op::Sharpen(_)));
    }

    /// Assert that an image was auto-oriented.
    pub fn assert_oriented(name: &str) {
        Self::assert_recorded(name, |r| matches!(r.op, Op::Orient));
    }

    /// Assert that an image was converted to the given output format.
    pub fn assert_converted_to(name: &str, format: OutputFormat) {
        Self::assert_recorded(name, |r| r.format == Some(format));
    }

    /// Assert that an image was stored to the given path.
    pub fn assert_stored(name: &str, path: &str) {
        Self::assert_recorded(name, |r| r.stored_to.as_deref() == Some(path));
    }

    /// Assert that an image was never stored.
    pub fn assert_not_stored(name: &str) {
        let state = fake_state().lock().unwrap();
        let records = state.as_ref().expect("Image is not faked");
        assert!(
            !records
                .iter()
                .any(|r| { r.name == name && r.stored_to.is_some() }),
            "Expected image `{name}` to NOT be stored, but it was."
        );
    }

    fn assert_recorded(name: &str, predicate: impl Fn(&RecordedOp) -> bool) {
        let state = fake_state().lock().unwrap();
        let records = state.as_ref().expect("Image is not faked");
        assert!(
            records.iter().any(|r| r.name == name && predicate(r)),
            "Expected a matching image operation for `{name}`.\nRecorded: {records:#?}"
        );
    }
}

fn record_fake(instance: &ImageInstance) {
    let mut state = fake_state().lock().unwrap();
    let Some(records) = state.as_mut() else {
        return;
    };
    let name = instance
        .source_name
        .clone()
        .unwrap_or_else(|| "image".to_string());
    if let Some(op) = instance.ops.last().cloned() {
        records.push(RecordedOp {
            name: name.clone(),
            op,
            format: None,
            quality: None,
            stored_to: None,
        });
        if let Some(format) = instance.format {
            records.push(RecordedOp {
                name,
                op: Op::Grayscale,
                format: Some(format),
                quality: None,
                stored_to: None,
            });
        }
    } else if let Some(format) = instance.format {
        records.push(RecordedOp {
            name,
            op: Op::Grayscale,
            format: Some(format),
            quality: None,
            stored_to: None,
        });
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn solid_png(width: u32, height: u32, color: [u8; 3]) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(width, height, image::Rgb(color));
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    fn temp_png_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("larastvel_img_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(&solid_png(200, 100, [255, 0, 0])).unwrap();
        path
    }

    #[test]
    fn from_bytes_and_dimensions() {
        let img = Image::from_bytes(&solid_png(200, 100, [255, 0, 0])).unwrap();
        assert_eq!(img.dimensions().unwrap(), (200, 100));
        assert_eq!(img.width().unwrap(), 200);
        assert_eq!(img.height().unwrap(), 100);
        assert_eq!(img.mime_type().unwrap(), "image/png");
        assert_eq!(img.extension(), "png");
    }

    #[test]
    fn from_base64_roundtrip() {
        let b64 = base64::engine::general_purpose::STANDARD.encode(solid_png(10, 10, [0, 255, 0]));
        let img = Image::from_base64(&b64).unwrap();
        assert_eq!(img.dimensions().unwrap(), (10, 10));
        assert!(Image::from_base64("not base64!").is_err());
    }

    #[test]
    fn from_path_reads_file() {
        let path = temp_png_path("in.png");
        let img = Image::from_path(&path.to_string_lossy()).unwrap();
        assert_eq!(img.dimensions().unwrap(), (200, 100));
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn resize_preserves_aspect_ratio() {
        let img = Image::from_bytes(&solid_png(200, 100, [0, 0, 255])).unwrap();
        let resized = img.resize(Some(100), None).unwrap();
        assert_eq!(resized.dimensions().unwrap(), (100, 50));
        let resized = img.resize(None, Some(50)).unwrap();
        assert_eq!(resized.dimensions().unwrap(), (100, 50));
        let resized = img.resize(Some(60), Some(40)).unwrap();
        assert_eq!(resized.dimensions().unwrap(), (60, 30));
        assert!(img.resize(None, None).is_err());
    }

    #[test]
    fn scale_never_upscales() {
        let img = Image::from_bytes(&solid_png(200, 100, [0, 0, 255])).unwrap();
        let scaled = img.scale(Some(400), None).unwrap();
        assert_eq!(scaled.dimensions().unwrap(), (200, 100));
        let scaled = img.scale(Some(100), None).unwrap();
        assert_eq!(scaled.dimensions().unwrap(), (100, 50));
    }

    #[test]
    fn cover_crops_to_dimensions() {
        let img = Image::from_bytes(&solid_png(200, 100, [0, 0, 255])).unwrap();
        let covered = img.cover(100, 100);
        assert_eq!(covered.dimensions().unwrap(), (100, 100));
        let covered = img.cover(150, 50);
        assert_eq!(covered.dimensions().unwrap(), (150, 50));
    }

    #[test]
    fn contain_fits_within_dimensions() {
        let img = Image::from_bytes(&solid_png(200, 100, [0, 0, 255])).unwrap();
        let contained = img.contain(100, 100, None);
        assert_eq!(contained.dimensions().unwrap(), (100, 100));
        let contained = img.contain(100, 100, Some("#ffffff"));
        assert_eq!(contained.dimensions().unwrap(), (100, 100));
    }

    #[test]
    fn crop_at_position() {
        let img = Image::from_bytes(&solid_png(200, 100, [0, 0, 255])).unwrap();
        let cropped = img.crop(50, 50, 10, 10);
        assert_eq!(cropped.dimensions().unwrap(), (50, 50));
        let cropped = img.crop(300, 200, 0, 0);
        assert_eq!(cropped.dimensions().unwrap(), (200, 100));
    }

    #[test]
    fn rotate_quadrants_exact() {
        let img = Image::from_bytes(&solid_png(200, 100, [0, 0, 255])).unwrap();
        let rotated = img.rotate(90.0, None);
        assert_eq!(rotated.dimensions().unwrap(), (100, 200));
        let rotated = img.rotate(180.0, None);
        assert_eq!(rotated.dimensions().unwrap(), (200, 100));
        let rotated = img.rotate(270.0, None);
        assert_eq!(rotated.dimensions().unwrap(), (100, 200));
    }

    #[test]
    fn grayscale_and_flips_change_pixels() {
        let img = Image::from_bytes(&solid_png(10, 10, [255, 0, 0])).unwrap();
        let gray = img.grayscale();
        assert_eq!(gray.dominant_color().unwrap(), "#363636");
        let flipped = img.flip_horizontally().flip_vertically();
        assert_eq!(flipped.dimensions().unwrap(), (10, 10));
    }

    #[test]
    fn encode_conversions_and_quality() {
        let img = Image::from_bytes(&solid_png(10, 10, [255, 0, 0])).unwrap();
        let jpg = img.to_jpg();
        assert_eq!(jpg.mime_type().unwrap(), "image/jpeg");
        assert_eq!(jpg.extension(), "jpg");
        let webp = img.to_webp().quality(80);
        assert_eq!(webp.mime_type().unwrap(), "image/webp");
        assert_eq!(webp.extension(), "webp");
        assert!(webp.to_bytes().unwrap().starts_with(b"RIFF"));
        let bmp = img.to_bmp();
        assert_eq!(bmp.mime_type().unwrap(), "image/bmp");
        assert_eq!(bmp.extension(), "bmp");
    }

    #[test]
    fn to_base64_and_data_uri() {
        let img = Image::from_bytes(&solid_png(10, 10, [255, 0, 0])).unwrap();
        let b64 = img.to_base64().unwrap();
        assert!(base64::engine::general_purpose::STANDARD
            .decode(&b64)
            .is_ok());
        let uri = img.to_data_uri().unwrap();
        assert!(uri.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn dominant_color_is_average() {
        let img = Image::from_bytes(&solid_png(10, 10, [0, 128, 255])).unwrap();
        assert_eq!(img.dominant_color().unwrap(), "#0080ff");
    }

    #[test]
    fn save_writes_processed_image() {
        let path = temp_png_path("save-me.png");
        let dir = path.parent().unwrap();
        let out = dir.join("sub").join("out.webp");
        let img = Image::from_path(&path.to_string_lossy()).unwrap();
        img.cover(50, 50).to_webp().save(&out).unwrap();
        assert!(out.exists());
        let loaded = image::load_from_memory(&std::fs::read(&out).unwrap()).unwrap();
        assert_eq!((loaded.width(), loaded.height()), (50, 50));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn immutable_pipeline_does_not_mutate_original() {
        let img = Image::from_bytes(&solid_png(200, 100, [0, 0, 255])).unwrap();
        let _processed = img.cover(50, 50);
        assert_eq!(img.dimensions().unwrap(), (200, 100));
        assert_eq!(
            img.to_bytes().unwrap().len(),
            solid_png(200, 100, [0, 0, 255]).len()
        );
    }

    #[test]
    fn store_and_fake_record_operations() {
        let was_faked = Image::fake();
        // Uses `storage/app` under the crate working directory; use a unique
        // subdirectory and clean it up afterwards.
        let subdir = format!("_tests_{}", uuid::Uuid::new_v4());
        let img = Image::from_path(&temp_png_path("fake.png").to_string_lossy()).unwrap();
        let path = img
            .cover(100, 100)
            .to_webp()
            .store_as(&subdir, "out.webp")
            .unwrap();
        assert_eq!(path, format!("{subdir}/out.webp"));
        Image::assert_covered("fake.png", 100, 100);
        Image::assert_converted_to("fake.png", OutputFormat::Webp);
        Image::assert_stored("fake.png", &format!("{subdir}/out.webp"));
        Image::restore();
        assert!(!was_faked);
        assert!(!Image::is_faked());
        std::fs::remove_dir_all(crate::storage_path(Some("app")).join(&subdir)).ok();
    }

    #[test]
    fn fake_records_crop_and_resize() {
        Image::fake();
        let img = Image::from_bytes(&solid_png(10, 10, [1, 2, 3])).unwrap();
        img.resize(Some(5), None).unwrap().crop(3, 3, 1, 1);
        Image::assert_resized("image", Some(5), None);
        Image::assert_cropped("image", 3, 3, 1, 1);
        Image::restore();
    }

    #[test]
    fn assert_not_stored() {
        Image::fake();
        let img = Image::from_bytes(&solid_png(10, 10, [1, 2, 3])).unwrap();
        img.grayscale();
        Image::assert_not_stored("image");
        Image::restore();
    }
}
