use image::{GrayImage, Luma};

pub const WHITE: Luma<u8> = Luma([u8::MAX]);
pub const BLACK: Luma<u8> = Luma([u8::MIN]);

/// Bleed the given luma value outwards from any pixels that match it.
pub fn bleed(img: &GrayImage, luma: &Luma<u8>) -> GrayImage {
    let mut out = img.clone();
    for (x, y, pixel) in img.enumerate_pixels() {
        if *pixel != *luma {
            continue;
        }

        if x > 0 {
            out.put_pixel(x - 1, y, *pixel);
        }
        if x < img.width() - 1 {
            out.put_pixel(x + 1, y, *pixel);
        }
        if y > 0 {
            out.put_pixel(x, y - 1, *pixel);
        }
        if y < img.height() - 1 {
            out.put_pixel(x, y + 1, *pixel);
        }
    }

    out
}

/// Generates an image from two images where corresponding pixels in `compare`
/// that are darker than their counterpart in `base` show up with the luminosity
/// difference between the two. This is useful for determining where a
/// light-background form was filled out, for example.
///
/// Note that the sizes of the images must be equal.
///
/// ```
///         BASE                  COMPARE                 DIFF
/// ┌───────────────────┐  ┌───────────────────┐  ┌───────────────────┐
/// │                   │  │        █ █ ███    │  │        █ █ ███    │
/// │ █ █               │  │ █ █    ███  █     │  │        ███  █     │
/// │  █                │  │  █     █ █ ███    │  │        █ █ ███    │
/// │ █ █ █████████████ │  │ █ █ █████████████ │  │                   │
/// └───────────────────┘  └───────────────────┘  └───────────────────┘
/// ```
///
pub fn diff(
    base: &GrayImage,
    compare: &GrayImage,
) -> GrayImage {
    assert_eq!(base.dimensions(), compare.dimensions());

    let mut out = GrayImage::new(base.width(), base.height());

    base.enumerate_pixels().for_each(|(x, y, base_pixel)| {
        let compare_pixel = compare.get_pixel(x, y);
        let diff = if base_pixel.0[0] < compare_pixel.0[0] {
            u8::MIN
        } else {
            base_pixel.0[0] - compare_pixel.0[0]
        };

        out.put_pixel(x, y, Luma([u8::MAX - diff]));
    });

    out
}

/// Determines the number of pixels in an image that match the given luma.
pub fn count_pixels(img: &GrayImage, luma: &Luma<u8>) -> u32 {
    img.pixels().filter(|p| *p == luma).count() as u32
}

/// Determines the ratio of pixels in an image that match the given luma.
pub fn ratio(img: &GrayImage, luma: &Luma<u8>) -> f32 {
    let total = img.width() * img.height();
    count_pixels(img, luma) as f32 / total as f32
}
