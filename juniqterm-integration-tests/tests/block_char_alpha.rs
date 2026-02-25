use juniqterm_gpu_draw::GlyphAtlas;

#[test]
fn full_block_edge_alpha() {
    let mut atlas = GlyphAtlas::new(32.0);
    let glyph = atlas.get_or_insert('█');
    let w = glyph.width as usize;
    let h = glyph.height as usize;

    // 첫 번째 열, 마지막 열, 첫 번째 행, 마지막 행의 alpha 값 출력
    eprintln!("█ bitmap {}x{}", w, h);

    eprint!("첫 열: ");
    for y in 0..h.min(5) {
        eprint!("{} ", glyph.bitmap[y * w]);
    }
    eprintln!("...");

    eprint!("끝 열: ");
    for y in 0..h.min(5) {
        eprint!("{} ", glyph.bitmap[y * w + w - 1]);
    }
    eprintln!("...");

    eprint!("첫 행: ");
    for x in 0..w.min(5) {
        eprint!("{} ", glyph.bitmap[x]);
    }
    eprintln!("...");

    eprint!("끝 행: ");
    for x in 0..w.min(5) {
        eprint!("{} ", glyph.bitmap[(h - 1) * w + x]);
    }
    eprintln!("...");

    // 모든 픽셀이 255인지 확인
    let non_full = glyph.bitmap.iter().filter(|&&v| v < 255).count();
    eprintln!("alpha < 255인 픽셀: {non_full}/{}", w * h);

    assert_eq!(non_full, 0, "█ 글리프에 alpha < 255 픽셀이 {non_full}개 있음");
}
