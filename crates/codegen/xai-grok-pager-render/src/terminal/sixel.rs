//! Sixel encoding for terminals whose graphics support is DCS-based.
//!
//! Windows Terminal 的图像适配使用 Sixel DCS；Kitty APC 与 iTerm2 OSC 1337
//! 是不同协议，不能直接替代此路径。
//!
//! The encoder quantizes to a 6x6x6 colour cube (216 registers, inside the 256 sixel
//! limit) and emits one colour pass per band of six rows, run-length encoded.

/// Pixels below this alpha are left unpainted so the terminal background shows through.
const ALPHA_THRESHOLD: u8 = 128;

/// Colour-cube resolution per channel; 6^3 = 216 registers.
const CUBE_LEVELS: u8 = 6;

const PALETTE_SIZE: usize =
    (CUBE_LEVELS as usize) * (CUBE_LEVELS as usize) * (CUBE_LEVELS as usize);

/// Rows each sixel band covers.
const BAND_ROWS: u32 = 6;

/// Runs shorter than this are cheaper as repeated characters than as `!N<ch>`.
const MIN_RUN: usize = 4;

/// Decode `image_data` and encode it as a sixel string fitted inside `target_px_w` x `target_px_h`.
pub fn sixel_for_image_bytes(
    image_data: &[u8],
    target_px_w: u32,
    target_px_h: u32,
) -> Option<String> {
    let img = image::ImageReader::new(std::io::Cursor::new(image_data))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    // Preserves the aspect ratio inside the target rect, matching the Kitty/iTerm2 paths.
    let fitted = img.resize(
        target_px_w.max(1),
        target_px_h.max(1),
        image::imageops::FilterType::Triangle,
    );
    let rgba = fitted.to_rgba8();
    encode_sixel(rgba.as_raw(), rgba.width(), rgba.height())
}

/// Encode straight RGBA pixels as a sixel DCS string.
///
/// Returns `None` for empty input, a short buffer, or a fully transparent image.
pub fn encode_sixel(rgba: &[u8], width: u32, height: u32) -> Option<String> {
    if width == 0 || height == 0 {
        return None;
    }
    let pixel_count = (width as usize).checked_mul(height as usize)?;
    if rgba.len() < pixel_count.checked_mul(4)? {
        return None;
    }

    let mut indices = vec![u8::MAX; pixel_count];
    let mut used = [false; PALETTE_SIZE];
    let (pixels, _trailing) = rgba.as_chunks::<4>();
    for (slot, [r, g, b, a]) in indices.iter_mut().zip(pixels.iter().copied()) {
        if a < ALPHA_THRESHOLD {
            continue;
        }
        let idx = cube_index(r, g, b);
        *slot = idx;
        if let Some(flag) = used.get_mut(usize::from(idx)) {
            *flag = true;
        }
    }

    let palette: Vec<u8> = (0..PALETTE_SIZE as u8)
        .filter(|idx| used.get(usize::from(*idx)).copied().unwrap_or(false))
        .collect();
    if palette.is_empty() {
        return None;
    }

    let mut out = String::with_capacity(pixel_count / 2 + 64);
    // P2=1 保留透明像素下的终端内容；默认 P2=0 会用颜色寄存器 0 填满背景。
    // `"1;1;w;h` 指定正方形虚拟像素及图像范围。
    out.push_str(&format!("\x1bP0;1q\"1;1;{width};{height}"));
    for idx in &palette {
        let (r, g, b) = cube_rgb(*idx);
        // Palette values are percentages (0-100) in sixel.
        out.push_str(&format!(
            "#{idx};2;{};{};{}",
            u16::from(r) * 100 / 255,
            u16::from(g) * 100 / 255,
            u16::from(b) * 100 / 255,
        ));
    }

    let mut band_bits = vec![0u8; width as usize];
    let mut y = 0;
    while y < height {
        let band_rows = BAND_ROWS.min(height - y);
        let mut painted = false;
        for color in &palette {
            for (x, bits) in band_bits.iter_mut().enumerate() {
                let mut value = 0u8;
                for row in 0..band_rows {
                    let offset = (y + row) as usize * width as usize + x;
                    if indices.get(offset).copied() == Some(*color) {
                        value |= 1 << row;
                    }
                }
                *bits = value;
            }
            // Colours absent from this band cost nothing when skipped.
            if band_bits.iter().all(|bits| *bits == 0) {
                continue;
            }
            out.push('#');
            out.push_str(&color.to_string());
            push_band(&mut out, &band_bits);
            out.push('$');
            painted = true;
        }
        // The band's final `$` becomes the band separator.
        if painted {
            out.pop();
        }
        out.push('-');
        y += BAND_ROWS;
    }
    // The last band separator is not needed.
    out.pop();
    out.push_str("\x1b\\");
    Some(out)
}

/// Sixel colour register for an RGB triple: 6 levels per channel.
fn cube_index(r: u8, g: u8, b: u8) -> u8 {
    let level = |v: u8| (u16::from(v) * u16::from(CUBE_LEVELS) / 256) as u8;
    level(r) * CUBE_LEVELS * CUBE_LEVELS + level(g) * CUBE_LEVELS + level(b)
}

/// Representative RGB for a cube register (level * 51).
fn cube_rgb(idx: u8) -> (u8, u8, u8) {
    let step = 255 / (CUBE_LEVELS - 1);
    let idx = u16::from(idx);
    let b = (idx % u16::from(CUBE_LEVELS)) as u8;
    let g = ((idx / u16::from(CUBE_LEVELS)) % u16::from(CUBE_LEVELS)) as u8;
    let r = (idx / (u16::from(CUBE_LEVELS) * u16::from(CUBE_LEVELS))) as u8;
    (r * step, g * step, b * step)
}

/// Emit one colour pass as sixel characters (`?` is empty, `~` is all six rows).
fn push_band(out: &mut String, bits: &[u8]) {
    let mut iter = bits.iter().copied().peekable();
    while let Some(value) = iter.next() {
        let mut run = 1;
        while iter.peek() == Some(&value) {
            iter.next();
            run += 1;
        }
        let ch = char::from(63 + value);
        if run >= MIN_RUN {
            out.push('!');
            out.push_str(&run.to_string());
            out.push(ch);
        } else {
            for _ in 0..run {
                out.push(ch);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
        rgba.iter()
            .copied()
            .cycle()
            .take(width as usize * height as usize * 4)
            .collect()
    }

    #[test]
    fn header_carries_raster_size_and_terminator() {
        let out = encode_sixel(&solid(3, 2, [255, 0, 0, 255]), 3, 2).unwrap();
        assert!(out.starts_with("\x1bP0;1q\"1;1;3;2"), "{out}");
        assert!(out.ends_with("\x1b\\"), "{out}");
        assert!(out.contains("#"), "palette must be defined: {out}");
    }

    #[test]
    fn solid_image_uses_one_register_and_run_length() {
        let out = encode_sixel(&solid(8, 1, [0, 0, 0, 255]), 8, 1).unwrap();
        assert_eq!(out.matches(";2;").count(), 1, "one palette entry: {out}");
        // Eight identical columns collapse into a single run; `@` sets the first row bit.
        assert!(out.contains("!8@"), "{out}");
    }

    #[test]
    fn transparent_pixels_stay_unpainted() {
        // One fully transparent pixel, then one opaque white pixel.
        let px = [0, 0, 0, 0, 255, 255, 255, 255];
        let out = encode_sixel(&px, 2, 1).unwrap();
        assert!(
            out.starts_with("\x1bP0;1q"),
            "透明像素需要显式禁止背景填充: {out:?}"
        );
        // Only the opaque column sets a bit, so its pass starts with an empty cell.
        assert!(out.contains("?@"), "first column must be blank: {out}");
    }

    #[test]
    fn fully_transparent_image_encodes_to_none() {
        assert!(encode_sixel(&solid(4, 4, [0, 0, 0, 0]), 4, 4).is_none());
    }

    #[test]
    fn short_buffer_is_rejected() {
        assert!(encode_sixel(&[0; 7], 2, 2).is_none());
        assert!(encode_sixel(&[], 0, 0).is_none());
    }

    #[test]
    fn cube_round_trips_primary_colours() {
        assert_eq!(cube_rgb(cube_index(255, 0, 0)), (255, 0, 0));
        assert_eq!(cube_rgb(cube_index(0, 255, 0)), (0, 255, 0));
        assert_eq!(cube_rgb(cube_index(0, 0, 255)), (0, 0, 255));
    }

    #[test]
    fn band_separator_is_emitted_per_six_rows() {
        let out = encode_sixel(&solid(1, 12, [0, 0, 0, 255]), 1, 12).unwrap();
        // Two bands -> one separator between them.
        assert_eq!(out.matches('-').count(), 1, "{out}");
    }
}
