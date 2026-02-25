use std::collections::HashMap;

pub struct RasterizedGlyph {
    pub width: u32,
    pub height: u32,
    pub bitmap: Vec<u8>,
    pub offset_x: f32,
    pub offset_y: f32,
}

pub struct GlyphAtlas {
    font: fontdue::Font,
    fallback_font: fontdue::Font,
    size: f32,
    cache: HashMap<char, RasterizedGlyph>,
    cell_width: f32,
    cell_height: f32,
    ascent: f32,
}

impl GlyphAtlas {
    pub fn new(size: f32) -> Self {
        let font_data = include_bytes!("../fonts/FiraCodeNerdFontMono-Retina.ttf");
        let settings = fontdue::FontSettings {
            scale: size,
            ..Default::default()
        };
        let font = fontdue::Font::from_bytes(font_data as &[u8], settings)
            .expect("failed to load Fira Code Nerd Font");

        let fallback_data = include_bytes!("../fonts/D2Coding.ttc");
        let fallback_settings = fontdue::FontSettings {
            scale: size,
            collection_index: 0,
            ..Default::default()
        };
        let fallback_font = fontdue::Font::from_bytes(fallback_data as &[u8], fallback_settings)
            .expect("failed to load D2Coding fallback font");

        let metrics = font.metrics('M', size);
        let line_metrics = font.horizontal_line_metrics(size);
        let (cell_height, ascent) = match line_metrics {
            Some(lm) => (lm.new_line_size, lm.ascent),
            None => (metrics.height as f32, metrics.height as f32 * 0.8),
        };

        Self {
            font,
            fallback_font,
            size,
            cache: HashMap::new(),
            cell_width: metrics.advance_width.ceil(),
            cell_height: cell_height.ceil(),
            ascent,
        }
    }

    pub fn set_size(&mut self, size: f32) {
        self.size = size;
        self.cache.clear();

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

    fn pick_font(&self, c: char) -> &fontdue::Font {
        if self.font.lookup_glyph_index(c) != 0 {
            &self.font
        } else {
            &self.fallback_font
        }
    }

    pub fn get_or_insert(&mut self, c: char) -> &RasterizedGlyph {
        if !self.cache.contains_key(&c) {
            let font = self.pick_font(c);
            let (metrics, bitmap) = font.rasterize(c, self.size);
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
}
