use growterm_gpu_draw::GlyphAtlas;

#[test]
fn print_cell_sizes() {
    for size in [24.0, 28.0, 32.0, 36.0, 40.0] {
        let atlas = GlyphAtlas::new(size);
        let (w, h) = atlas.cell_size();
        let w_frac = w - w.floor();
        let h_frac = h - h.floor();
        eprintln!(
            "font_size={size}: cell_w={w:.4} (frac={w_frac:.4}), cell_h={h:.4} (frac={h_frac:.4})"
        );
    }
}
