use growterm_grid::Grid;
use growterm_render_cmd::{generate, TerminalPalette};
use growterm_types::Rgb;
use growterm_vt_parser::VtParser;

fn render(input: &[u8], palette: TerminalPalette) -> Vec<growterm_types::RenderCommand> {
    let mut parser = VtParser::new();
    let commands = parser.parse(input);
    let mut grid = Grid::new(16, 1);
    for command in &commands {
        grid.apply(command);
    }
    generate(grid.cells(), None, None, None, palette)
}

#[test]
fn default_colors_follow_injected_palette() {
    let palette = TerminalPalette {
        default_fg: Rgb::new(12, 34, 56),
        default_bg: Rgb::new(65, 43, 21),
    };

    // A: explicit FG, B: default FG, C: explicit BG, D: default BG
    let cmds = render(
        b"\x1b[38;2;200;1;2mA\x1b[39mB\x1b[48;2;3;4;5mC\x1b[49mD",
        palette,
    );

    assert_eq!(cmds[0].character, 'A');
    assert_eq!(cmds[0].fg, Rgb::new(200, 1, 2));

    assert_eq!(cmds[1].character, 'B');
    assert_eq!(cmds[1].fg, palette.default_fg);

    assert_eq!(cmds[2].character, 'C');
    assert_eq!(cmds[2].bg, Rgb::new(3, 4, 5));

    assert_eq!(cmds[3].character, 'D');
    assert_eq!(cmds[3].bg, palette.default_bg);
}
