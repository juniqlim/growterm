//! The one place that asks the OS which font files might cover a character.
//!
//! Everything downstream — caching, coverage checks, rasterization — is
//! platform-independent and lives in `atlas`.

use std::path::PathBuf;

/// Font files that may contain a glyph for `c`, best candidate first.
pub type FontCandidates = fn(char, f32) -> Vec<PathBuf>;

/// Ask fontconfig. It always answers, so the caller still verifies coverage.
#[cfg(not(target_os = "macos"))]
pub fn candidates(c: char, _size: f32) -> Vec<PathBuf> {
    let output = std::process::Command::new("fc-match")
        .arg("--format=%{file}")
        .arg(format!(":charset={:x}", c as u32))
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if path.is_empty() {
                Vec::new()
            } else {
                vec![PathBuf::from(path)]
            }
        }
        _ => Vec::new(),
    }
}

/// Walk Core Text's cascade list, keeping fonts that report a glyph for `c`.
#[cfg(target_os = "macos")]
pub fn candidates(c: char, size: f32) -> Vec<PathBuf> {
    use core_foundation::array::CFArray;
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use core_text::font as ct_font;
    use core_text::font::CTFontRef;

    let Ok(base) = ct_font::new_from_name("Helvetica", size as f64) else {
        return Vec::new();
    };
    let langs: CFArray<CFString> = CFArray::from_CFTypes(&[]);
    let cascade = ct_font::cascade_list_for_languages(&base, &langs);

    let mut utf16_buf = [0u16; 2];
    let utf16 = c.encode_utf16(&mut utf16_buf);
    let mut glyph_buf = [0u16; 2];
    let mut paths = Vec::new();

    for i in 0..cascade.len() {
        let Some(descriptor) = cascade.get(i) else {
            continue;
        };
        let candidate = ct_font::new_from_descriptor(&descriptor, size as f64);

        let found = unsafe {
            extern "C" {
                fn CTFontGetGlyphsForCharacters(
                    font: CTFontRef,
                    characters: *const u16,
                    glyphs: *mut u16,
                    count: isize,
                ) -> bool;
            }
            CTFontGetGlyphsForCharacters(
                candidate.as_concrete_TypeRef(),
                utf16.as_ptr(),
                glyph_buf.as_mut_ptr(),
                utf16.len() as isize,
            )
        };

        if found && glyph_buf[0] != 0 {
            if let Some(url) = candidate.url() {
                if let Some(path) = url.to_path() {
                    paths.push(path.to_path_buf());
                }
            }
        }
    }
    paths
}
