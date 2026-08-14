use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::platform_font::{self, FontCandidates};

pub struct RasterizedGlyph {
    pub width: u32,
    pub height: u32,
    pub bitmap: Vec<u8>,
    pub offset_x: f32,
    pub offset_y: f32,
}

pub struct GlyphAtlas {
    font: Arc<fontdue::Font>,
    fallback_font: Arc<fontdue::Font>,
    bold_font: Arc<fontdue::Font>,
    bold_fallback_font: Arc<fontdue::Font>,
    system_font_cache: HashMap<PathBuf, fontdue::Font>,
    char_to_font_path: HashMap<char, PathBuf>,
    size: f32,
    cache: HashMap<char, RasterizedGlyph>,
    bold_cache: HashMap<char, RasterizedGlyph>,
    cell_width: f32,
    cell_height: f32,
    ascent: f32,
    /// How the OS is asked for fallback font files. Swapped in tests.
    font_candidates: FontCandidates,
}

impl GlyphAtlas {
    pub fn new(size: f32, font_path: Option<&str>) -> Self {
        Self::with_candidates(size, font_path, platform_font::candidates)
    }

    /// Same as `new`, with the OS font lookup supplied by the caller.
    pub fn with_candidates(size: f32, font_path: Option<&str>, font_candidates: FontCandidates) -> Self {
        let font = Arc::new(Self::load_font(size, font_path));
        let fallback_font = Arc::new(Self::load_fallback_font(size));
        let bold_font = Arc::new(Self::load_builtin_bold_font(size));
        let bold_fallback_font = Arc::new(Self::load_fallback_bold_font(size));
        let mut atlas =
            Self::with_shared_fonts(size, font, fallback_font, bold_font, bold_fallback_font);
        atlas.font_candidates = font_candidates;
        atlas
    }

    pub fn with_shared_fonts(size: f32, font: Arc<fontdue::Font>, fallback_font: Arc<fontdue::Font>, bold_font: Arc<fontdue::Font>, bold_fallback_font: Arc<fontdue::Font>) -> Self {
        let metrics = font.metrics('M', size);
        let line_metrics = font.horizontal_line_metrics(size);
        let (cell_height, ascent) = match line_metrics {
            Some(lm) => (lm.new_line_size, lm.ascent),
            None => (metrics.height as f32, metrics.height as f32 * 0.8),
        };

        Self {
            font,
            fallback_font,
            bold_font,
            bold_fallback_font,
            system_font_cache: HashMap::new(),
            char_to_font_path: HashMap::new(),
            size,
            cache: HashMap::new(),
            bold_cache: HashMap::new(),
            cell_width: metrics.advance_width.ceil(),
            cell_height: cell_height.ceil(),
            ascent,
            font_candidates: platform_font::candidates,
        }
    }

    pub fn load_font(size: f32, font_path: Option<&str>) -> fontdue::Font {
        if let Some(path) = font_path {
            if let Ok(data) = std::fs::read(path) {
                let settings = fontdue::FontSettings {
                    scale: size,
                    ..Default::default()
                };
                return fontdue::Font::from_bytes(data, settings).unwrap_or_else(|_| {
                    Self::load_builtin_font(size)
                });
            }
        }
        Self::load_builtin_font(size)
    }

    pub fn load_fallback_font(size: f32) -> fontdue::Font {
        let fallback_data = include_bytes!("../fonts/D2Coding.ttc");
        let fallback_settings = fontdue::FontSettings {
            scale: size,
            collection_index: 0,
            ..Default::default()
        };
        fontdue::Font::from_bytes(fallback_data as &[u8], fallback_settings)
            .expect("failed to load D2Coding fallback font")
    }

    pub fn load_builtin_font(size: f32) -> fontdue::Font {
        let font_data = include_bytes!("../fonts/FiraCodeNerdFontMono-Retina.ttf");
        let settings = fontdue::FontSettings {
            scale: size,
            ..Default::default()
        };
        fontdue::Font::from_bytes(font_data as &[u8], settings)
            .expect("failed to load Fira Code Nerd Font")
    }

    pub fn load_builtin_bold_font(size: f32) -> fontdue::Font {
        let font_data = include_bytes!("../fonts/FiraCodeNerdFontMono-Bold.ttf");
        let settings = fontdue::FontSettings {
            scale: size,
            ..Default::default()
        };
        fontdue::Font::from_bytes(font_data as &[u8], settings)
            .expect("failed to load Fira Code Nerd Font Bold")
    }

    pub fn load_fallback_bold_font(size: f32) -> fontdue::Font {
        let fallback_data = include_bytes!("../fonts/D2CodingBold.ttf");
        let settings = fontdue::FontSettings {
            scale: size,
            ..Default::default()
        };
        fontdue::Font::from_bytes(fallback_data as &[u8], settings)
            .expect("failed to load D2Coding Bold fallback font")
    }

    pub fn set_font(&mut self, font_path: Option<&str>, size: f32) {
        self.font = Arc::new(Self::load_font(size, font_path));
        self.size = size;
        self.cache.clear();
        self.bold_cache.clear();
        self.system_font_cache.clear();
        self.char_to_font_path.clear();
        let metrics = self.font.metrics('M', size);
        let line_metrics = self.font.horizontal_line_metrics(size);
        match line_metrics {
            Some(lm) => { self.cell_height = lm.new_line_size.ceil(); self.ascent = lm.ascent; }
            None => { self.cell_height = (metrics.height as f32).ceil(); self.ascent = metrics.height as f32 * 0.8; }
        }
        self.cell_width = metrics.advance_width.ceil();
    }

    pub fn set_size(&mut self, size: f32) {
        self.size = size;
        self.cache.clear();
        self.bold_cache.clear();
        self.system_font_cache.clear();
        self.char_to_font_path.clear();

        let metrics = self.font.metrics('M', size);
        let line_metrics = self.font.horizontal_line_metrics(size);
        match line_metrics {
            Some(lm) => { self.cell_height = lm.new_line_size.ceil(); self.ascent = lm.ascent; }
            None => { self.cell_height = (metrics.height as f32).ceil(); self.ascent = metrics.height as f32 * 0.8; }
        }
        self.cell_width = metrics.advance_width.ceil();
    }

    pub fn cell_size(&self) -> (f32, f32) {
        (self.cell_width, self.cell_height)
    }

    pub fn ascent(&self) -> f32 {
        self.ascent
    }

    /// Locate a system font that covers `c` and cache it so `get_or_insert`
    /// can rasterize the glyph. The OS only supplies candidate paths; coverage
    /// is verified here because a candidate may not actually contain `c`
    /// (fontconfig always answers, and color-emoji fonts fontdue cannot
    /// outline are rejected at load).
    fn find_system_font(&mut self, c: char) -> bool {
        if self.char_to_font_path.contains_key(&c) {
            return true;
        }

        // An already-loaded font may cover it, which saves asking the OS.
        for (path, font) in &self.system_font_cache {
            if font.lookup_glyph_index(c) != 0 {
                let path = path.clone();
                self.char_to_font_path.insert(c, path);
                return true;
            }
        }

        for path in (self.font_candidates)(c, self.size) {
            if !self.system_font_cache.contains_key(&path) {
                let Ok(data) = std::fs::read(&path) else {
                    continue;
                };
                let settings = fontdue::FontSettings {
                    scale: self.size,
                    ..Default::default()
                };
                let Ok(font) = fontdue::Font::from_bytes(data, settings) else {
                    continue;
                };
                // Cache regardless of coverage, to avoid re-reading the file.
                self.system_font_cache.insert(path.clone(), font);
            }
            if self.system_font_cache[&path].lookup_glyph_index(c) != 0 {
                self.char_to_font_path.insert(c, path);
                return true;
            }
        }
        false
    }


    /// fontdue는 힌팅/스템 다크닝이 없어 안티앨리어싱 획이 얇고 흐리게(뿌옇게) 보인다.
    /// 커버리지에 감마(<1.0) 곡선을 적용해 부분 커버리지 픽셀을 진하게 만들어,
    /// FreeType 렌더링에 가깝게 획을 또렷하게 보정한다. 0/255 끝값은 그대로 둔다.
    fn darken_coverage(a: u8) -> u8 {
        const STEM_GAMMA: f32 = 0.72;
        if a == 0 || a == 255 {
            return a;
        }
        let x = a as f32 / 255.0;
        (x.powf(STEM_GAMMA) * 255.0).round() as u8
    }

    fn darkened(bitmap: Vec<u8>) -> Vec<u8> {
        bitmap.into_iter().map(Self::darken_coverage).collect()
    }

    pub fn get_or_insert(&mut self, c: char) -> &RasterizedGlyph {
        if !self.cache.contains_key(&c) {
            // find_system_font borrows &mut self, so call it before taking &self refs
            let system_font_path = if self.font.lookup_glyph_index(c) != 0 || self.fallback_font.lookup_glyph_index(c) != 0 {
                None
            } else if self.find_system_font(c) {
                Some(self.char_to_font_path.get(&c).unwrap().clone())
            } else {
                None
            };

            let font: &fontdue::Font = if self.font.lookup_glyph_index(c) != 0 {
                &self.font
            } else if self.fallback_font.lookup_glyph_index(c) != 0 {
                &self.fallback_font
            } else if let Some(ref path) = system_font_path {
                self.system_font_cache.get(path).unwrap()
            } else {
                &self.font
            };

            let (metrics, bitmap) = font.rasterize(c, self.size);
            let bitmap = Self::darkened(bitmap);
            self.cache.insert(c, RasterizedGlyph {
                width: metrics.width as u32,
                height: metrics.height as u32,
                bitmap,
                offset_x: metrics.xmin as f32,
                offset_y: metrics.ymin as f32,
            });
        }
        self.cache.get(&c).unwrap()
    }

    pub fn get_or_insert_bold(&mut self, c: char) -> &RasterizedGlyph {
        if !self.bold_cache.contains_key(&c) {
            let font: &fontdue::Font = if self.bold_font.lookup_glyph_index(c) != 0 {
                &self.bold_font
            } else if self.bold_fallback_font.lookup_glyph_index(c) != 0 {
                &self.bold_fallback_font
            } else if self.font.lookup_glyph_index(c) != 0 {
                // Fallback to normal font if no bold variant has this glyph
                &self.font
            } else if self.fallback_font.lookup_glyph_index(c) != 0 {
                &self.fallback_font
            } else {
                &self.font
            };

            let (metrics, bitmap) = font.rasterize(c, self.size);
            let bitmap = Self::darkened(bitmap);
            self.bold_cache.insert(c, RasterizedGlyph {
                width: metrics.width as u32,
                height: metrics.height as u32,
                bitmap,
                offset_x: metrics.xmin as f32,
                offset_y: metrics.ymin as f32,
            });
        }
        self.bold_cache.get(&c).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bold_glyph_differs_from_normal() {
        let size = 16.0;
        let normal_font = GlyphAtlas::load_builtin_font(size);
        let bold_font = GlyphAtlas::load_builtin_bold_font(size);

        let (_, normal_bitmap) = normal_font.rasterize('A', size);
        let (_, bold_bitmap) = bold_font.rasterize('A', size);

        assert_ne!(normal_bitmap, bold_bitmap, "Bold glyph should differ from normal");
    }

    #[test]
    fn bold_fallback_glyph_differs_from_normal() {
        let size = 16.0;
        let normal_font = GlyphAtlas::load_fallback_font(size);
        let bold_font = GlyphAtlas::load_fallback_bold_font(size);

        let (_, normal_bitmap) = normal_font.rasterize('가', size);
        let (_, bold_bitmap) = bold_font.rasterize('가', size);

        assert_ne!(normal_bitmap, bold_bitmap, "Bold Korean glyph should differ from normal");
    }

    #[test]
    fn darken_coverage_preserves_endpoints_and_thickens_midtones() {
        assert_eq!(GlyphAtlas::darken_coverage(0), 0, "완전 투명은 유지");
        assert_eq!(GlyphAtlas::darken_coverage(255), 255, "완전 불투명은 유지");
        assert!(
            GlyphAtlas::darken_coverage(128) > 128,
            "중간 커버리지는 진해져야 함"
        );

        // 단조 증가 확인
        let mut prev = 0;
        for a in 0..=255u8 {
            let v = GlyphAtlas::darken_coverage(a);
            assert!(v >= prev, "커버리지 보정은 단조 증가여야 함: a={a}");
            prev = v;
        }
    }

    #[test]
    fn get_or_insert_darkens_glyph_coverage() {
        let size = 32.0;
        let raw = GlyphAtlas::load_builtin_font(size);
        let (_, raw_bitmap) = raw.rasterize('A', size);
        let raw_sum: u64 = raw_bitmap.iter().map(|&b| b as u64).sum();

        let mut atlas = GlyphAtlas::new(size, None);
        let g = atlas.get_or_insert('A');
        let darkened_sum: u64 = g.bitmap.iter().map(|&b| b as u64).sum();

        assert!(
            darkened_sum > raw_sum,
            "스템 다크닝 적용 후 총 커버리지가 커야 함: {darkened_sum} > {raw_sum}"
        );
    }

    #[test]
    fn get_or_insert_bold_returns_different_glyph() {
        let size = 16.0;
        let mut atlas = GlyphAtlas::new(size, None);

        let normal = atlas.get_or_insert('A');
        let normal_bitmap = normal.bitmap.clone();

        let bold = atlas.get_or_insert_bold('A');
        let bold_bitmap = bold.bitmap.clone();

        assert_ne!(normal_bitmap, bold_bitmap, "Bold cached glyph should differ from normal");
    }

    // System font lookup used to have one implementation per OS, so neither
    // could be tested off its own platform. The OS now only supplies candidate
    // paths; these cover the caching and coverage logic on any machine.

    const HANGUL_FONT: &str = "fonts/D2Coding.ttc";
    const LATIN_ONLY_FONT: &str = "fonts/FiraCodeNerdFontMono-Retina.ttf";

    fn atlas_with(candidates: FontCandidates) -> GlyphAtlas {
        GlyphAtlas::with_candidates(16.0, None, candidates)
    }

    #[test]
    fn find_system_font_fails_when_the_os_offers_nothing() {
        let mut atlas = atlas_with(|_, _| Vec::new());
        assert!(!atlas.find_system_font('가'));
    }

    #[test]
    fn find_system_font_accepts_a_candidate_covering_the_char() {
        let mut atlas = atlas_with(|_, _| vec![PathBuf::from(HANGUL_FONT)]);
        assert!(atlas.find_system_font('가'));
        assert_eq!(atlas.char_to_font_path.get(&'가'), Some(&PathBuf::from(HANGUL_FONT)));
    }

    #[test]
    fn find_system_font_skips_a_candidate_missing_the_glyph() {
        let mut atlas = atlas_with(|_, _| {
            vec![PathBuf::from(LATIN_ONLY_FONT), PathBuf::from(HANGUL_FONT)]
        });
        assert!(atlas.find_system_font('가'));
        assert_eq!(atlas.char_to_font_path.get(&'가'), Some(&PathBuf::from(HANGUL_FONT)));
    }

    #[test]
    fn find_system_font_ignores_a_candidate_that_cannot_be_read() {
        let mut atlas = atlas_with(|_, _| vec![PathBuf::from("/nonexistent/font.ttf")]);
        assert!(!atlas.find_system_font('가'));
    }

    #[test]
    fn find_system_font_reuses_an_already_loaded_font() {
        let mut atlas = atlas_with(|_, _| vec![PathBuf::from(HANGUL_FONT)]);
        assert!(atlas.find_system_font('가'));

        // The OS is not consulted again: this would panic if it were.
        atlas.font_candidates = |_, _| panic!("should not ask the OS again");
        assert!(atlas.find_system_font('힣'));
    }
}




