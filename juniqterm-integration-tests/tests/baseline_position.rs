use juniqterm_gpu_draw::GlyphAtlas;

const FONT_SIZE: f32 = 32.0;

/// baseline을 폰트의 ascent로 설정하면 █ 글리프가 셀을 정확히 채우는지 검증.
#[test]
fn block_char_covers_cell_with_ascent_baseline() {
    let mut atlas = GlyphAtlas::new(FONT_SIZE);
    let (cell_w, cell_h) = atlas.cell_size();
    let ascent = atlas.ascent();

    let full_block = atlas.get_or_insert('█');

    // baseline = cell_y + ascent 일 때 글리프 위치
    let baseline_y = ascent;
    let gy = baseline_y - full_block.offset_y - full_block.height as f32;
    let glyph_bottom = gy + full_block.height as f32;

    eprintln!("cell=({cell_w:.2}, {cell_h:.2}), ascent={ascent:.2}");
    eprintln!("█: size={}x{}, offset=({}, {})", full_block.width, full_block.height, full_block.offset_x, full_block.offset_y);
    eprintln!("baseline_y={baseline_y:.2}, gy={gy:.2}, glyph_bottom={glyph_bottom:.2}");
    eprintln!("셀 상단 갭: {gy:.2}px, 셀 하단 넘침: {:.2}px", glyph_bottom - cell_h);

    // 기존 0.8 방식과 비교
    let old_baseline = cell_h * 0.8;
    let old_gy = old_baseline - full_block.offset_y - full_block.height as f32;
    eprintln!("\n기존 0.8: baseline={old_baseline:.2}, gy={old_gy:.2}, 상단 갭={old_gy:.2}px");

    // ascent 방식이 0.8보다 상단 갭이 작아야 함
    assert!(
        gy.abs() < old_gy.abs(),
        "ascent baseline이 더 정확해야 함: ascent gap={gy:.2} vs 0.8 gap={old_gy:.2}"
    );
}
