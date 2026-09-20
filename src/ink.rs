//! Did the capture actually paint anything?
//!
//! A blank-but-valid PNG is byte-for-byte a success: same encoder, same
//! dimensions, exit code zero. It is what a failed hydration, a JS error
//! swallowed to undefined, or a `waitForSelector` that matched a skeleton
//! loader all produce. Measuring the pixels is the only way to tell those from
//! a page that genuinely renders as an empty white box.

/// Returns (fraction of non-background pixels, distinct colour count).
///
/// "Background" is whichever colour dominates rather than white specifically:
/// a dark-themed app that failed to hydrate is uniformly dark, and calling that
/// 100% ink would invert the signal.
pub fn coverage(png: &[u8]) -> (f64, usize) {
    let Ok(img) = image::load_from_memory_with_format(png, image::ImageFormat::Png) else {
        return (0.0, 0);
    };
    let rgb = img.to_rgb8();
    let total = (rgb.width() as usize) * (rgb.height() as usize);
    if total == 0 {
        return (0.0, 0);
    }
    let mut counts: std::collections::HashMap<[u8; 3], usize> = std::collections::HashMap::new();
    for px in rgb.pixels() {
        *counts.entry(px.0).or_insert(0) += 1;
    }
    let dominant = counts.values().copied().max().unwrap_or(0);
    ((total - dominant) as f64 / total as f64, counts.len())
}
