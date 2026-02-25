use juniqterm_gpu_draw::GlyphAtlas;

const FONT_SIZE: f32 = 32.0;

/// 블록 문자(█)의 글리프 비트맵이 셀을 완전히 채우는지 검사한다.
/// 글리프가 셀보다 작으면 배경이 비쳐서 보더라인이 보인다.
#[test]
fn block_char_fills_cell() {
    let mut atlas = GlyphAtlas::new(FONT_SIZE);
    let (cell_w, cell_h) = atlas.cell_size();

    let block_chars = ['█', '▐', '▛', '▜', '▝', '▘', '▌'];

    for ch in block_chars {
        let glyph = atlas.get_or_insert(ch);
        eprintln!(
            "'{ch}' (U+{:04X}): glyph={}x{}, offset=({}, {}), cell=({cell_w:.2}, {cell_h:.2})",
            ch as u32, glyph.width, glyph.height, glyph.offset_x, glyph.offset_y
        );
    }

    // █ (FULL BLOCK)은 셀 전체를 채워야 한다
    let full_block = atlas.get_or_insert('█');
    let covers_width = full_block.width as f32 + full_block.offset_x >= cell_w;
    let covers_height = full_block.height as f32 >= cell_h;

    eprintln!(
        "\n█ 커버리지: width({})+ offset_x({}) = {} vs cell_w({cell_w:.2}), height({}) vs cell_h({cell_h:.2})",
        full_block.width, full_block.offset_x,
        full_block.width as f32 + full_block.offset_x,
        full_block.height,
    );
    eprintln!("가로 커버: {covers_width}, 세로 커버: {covers_height}");

    assert!(
        covers_width,
        "█ 글리프가 셀 가로를 못 채움: glyph_w({}) + offset_x({}) < cell_w({cell_w})",
        full_block.width, full_block.offset_x
    );
    assert!(
        covers_height,
        "█ 글리프가 셀 세로를 못 채움: glyph_h({}) < cell_h({cell_h})",
        full_block.height
    );
}
