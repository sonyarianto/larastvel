# Image Processing

Larastvel ships a first-party `Image` facade (tracking Laravel 13.20's
`Illuminate\Image`) built on the `image` crate. It provides an immutable
processing pipeline, multiple output formats, disk storage under
`storage/app`, and a full test fake.

## Reading images

```rust
use larastvel_core::Image;

// From raw bytes
let img = Image::from_bytes(&bytes)?;

// From base64
let img = Image::from_base64("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==")?;

// From a local path
let img = Image::from_path("public/uploads/photo.jpg")?;

// From the default storage disk
let img = Image::from_storage("photos/avatar.jpg")?;

// From a URL
let img = Image::from_url("https://example.com/photo.png").await?;
```

## Transformations

`ImageInstance` methods are immutable — each returns a new instance with the
operation queued, so calls can be chained:

```rust
let processed = Image::from_path("/tmp/photo.jpg")?
    .cover(400, 400)
    .grayscale()
    .to_jpg();
```

Available operations:

| Method | Description |
|---|---|
| `resize(w, h)` / `scale(w, h)` | Scale, preserving aspect ratio when one side is `None` |
| `cover(w, h)` | Resize and center-crop to fill exactly `w`×`h` |
| `contain(w, h, background?)` | Resize to fit inside `w`×`h`, filling the remainder |
| `crop(w, h, x, y)` | Crop a `w`×`h` region starting at `(x, y)` |
| `rotate(angle, background?)` | Rotate by degrees |
| `grayscale()` | Desaturate |
| `blur(amount)` / `sharpen(amount)` | Gaussian blur / sharpen |
| `flip()` / `flop()` / `flip_vertically()` / `flip_horizontally()` | Mirroring |
| `orient()` | Auto-orient by EXIF (recorded in the pipeline; applied where the decoder exposes orientation) |
| `quality(u8)` | Output quality (1–100) for lossy formats |

## Output

```rust
let bytes = img.to_bytes()?;        // Vec<u8> in the current format
let base64 = img.to_base64()?;      // base64-encoded
let data_uri = img.to_data_uri()?;  // data:image/...;base64,...

let png = img.to_png();
let webp = img.to_webp();
let jpg = img.to_jpg();             // alias: to_jpeg()
let gif = img.to_gif();
let bmp = img.to_bmp();
```

Inspect the result:

```rust
let (w, h) = img.dimensions()?;
let color = img.dominant_color()?;  // "#rrggbb"
let mime = img.mime_type()?;        // e.g. "image/png"
let ext = img.extension();          // e.g. "png"
```

## Storage

`save`/`store` write to disk with a hashed filename under `storage/app` using
the default storage disk:

```rust
let path = img.store_publicly("photos")?;   // storage/app/photos/<hash>.jpg
let path = img.save("/tmp/edited.jpg")?;    // explicit filesystem path
```

## Testing

`Image::fake()` swaps the encoder for a record-only fake that captures every
operation. Assertions inspect source names and operation arguments:

```rust
use larastvel_core::Image;

#[test]
async fn avatars_are_covered_and_cropped() {
    Image::fake();

    // ... run the code under test ...

    Image::assert_covered("avatar", 200, 200);
    Image::assert_cropped("avatar", 200, 200, 0, 0);
    Image::assert_rotated("avatar", 90.0);

    Image::restore();
}
```

Assertions are available for every operation: `assert_resized`,
`assert_covered`, `assert_contained`, `assert_cropped`, `assert_rotated`,
`assert_blurred`, `assert_sharpened`, `assert_grayscaled`, `assert_flipped`,
and `assert_stored`.