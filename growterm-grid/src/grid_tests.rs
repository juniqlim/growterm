use crate::{Grid, MAX_SCROLLBACK};
use growterm_types::{Cell, CellFlags, Color, Rgb, TerminalCommand};

// === Step 1: Grid::new + cells() ===

#[test]
fn new_grid_has_correct_dimensions() {
    let grid = Grid::new(80, 24);
    let cells = grid.cells();
    assert_eq!(cells.len(), 24);
    for row in cells {
        assert_eq!(row.len(), 80);
    }
}

#[test]
fn new_grid_all_cells_are_default() {
    let grid = Grid::new(10, 5);
    for row in grid.cells() {
        for cell in row {
            assert_eq!(*cell, Cell::default());
        }
    }
}

// === Step 2: Print(char) - ASCII ===

#[test]
fn print_ascii_places_char_at_cursor() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::Print('A'));
    assert_eq!(grid.cells()[0][0].character, 'A');
    // cursor should have advanced
    grid.apply(&TerminalCommand::Print('B'));
    assert_eq!(grid.cells()[0][1].character, 'B');
}

#[test]
fn print_ascii_wraps_at_end_of_line() {
    let mut grid = Grid::new(3, 2);
    grid.apply(&TerminalCommand::Print('A'));
    grid.apply(&TerminalCommand::Print('B'));
    grid.apply(&TerminalCommand::Print('C'));
    // Should wrap to next line
    grid.apply(&TerminalCommand::Print('D'));
    assert_eq!(grid.cells()[0][0].character, 'A');
    assert_eq!(grid.cells()[0][1].character, 'B');
    assert_eq!(grid.cells()[0][2].character, 'C');
    assert_eq!(grid.cells()[1][0].character, 'D');
}

#[test]
fn print_ascii_multiple_chars_sequence() {
    let mut grid = Grid::new(80, 24);
    for c in "Hello".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    assert_eq!(grid.cells()[0][0].character, 'H');
    assert_eq!(grid.cells()[0][1].character, 'e');
    assert_eq!(grid.cells()[0][2].character, 'l');
    assert_eq!(grid.cells()[0][3].character, 'l');
    assert_eq!(grid.cells()[0][4].character, 'o');
}

// === Step 3: Print(char) - Wide chars + spacer ===

#[test]
fn print_wide_char_sets_flag_and_spacer() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::Print('한'));
    assert_eq!(grid.cells()[0][0].character, '한');
    assert!(grid.cells()[0][0].flags.contains(CellFlags::WIDE_CHAR));
    // spacer cell at col 1
    assert_eq!(grid.cells()[0][1].character, ' ');
    assert!(grid.cells()[0][1].flags.is_empty());
}

#[test]
fn print_wide_char_cursor_advances_by_two() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::Print('한'));
    grid.apply(&TerminalCommand::Print('A'));
    assert_eq!(grid.cells()[0][2].character, 'A');
}

#[test]
fn wide_char_wraps_when_one_col_remaining() {
    let mut grid = Grid::new(3, 2);
    grid.apply(&TerminalCommand::Print('A')); // col 0
    grid.apply(&TerminalCommand::Print('B')); // col 1
    // 1 col remaining, wide char should wrap to next line
    grid.apply(&TerminalCommand::Print('한'));
    assert_eq!(grid.cells()[0][2].character, ' '); // col 2 stays default
    assert_eq!(grid.cells()[1][0].character, '한');
    assert!(grid.cells()[1][0].flags.contains(CellFlags::WIDE_CHAR));
    assert_eq!(grid.cells()[1][1].character, ' '); // spacer
}

#[test]
fn narrow_overwrite_on_wide_clears_spacer() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::Print('한')); // col 0 wide, col 1 spacer
    // Move cursor back to col 0
    grid.apply(&TerminalCommand::CursorPosition { row: 1, col: 1 });
    grid.apply(&TerminalCommand::Print('X')); // overwrite col 0
    assert_eq!(grid.cells()[0][0].character, 'X');
    assert!(!grid.cells()[0][0].flags.contains(CellFlags::WIDE_CHAR));
    // spacer at col 1 should be cleared to default
    assert_eq!(grid.cells()[0][1].character, ' ');
    assert!(grid.cells()[0][1].flags.is_empty());
}

#[test]
fn narrow_overwrite_on_spacer_clears_wide_char() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::Print('한')); // col 0 wide, col 1 spacer
    // Move cursor to col 1 (the spacer)
    grid.apply(&TerminalCommand::CursorPosition { row: 1, col: 2 });
    grid.apply(&TerminalCommand::Print('Y')); // overwrite col 1 (spacer)
    // col 0 should be cleared since its wide pair was broken
    assert_eq!(grid.cells()[0][0].character, ' ');
    assert!(!grid.cells()[0][0].flags.contains(CellFlags::WIDE_CHAR));
    assert_eq!(grid.cells()[0][1].character, 'Y');
}

// === Step 4: Attribute state ===

#[test]
fn set_foreground_applies_to_printed_cell() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::SetForeground(Color::Rgb(Rgb::new(255, 0, 0))));
    grid.apply(&TerminalCommand::Print('R'));
    assert_eq!(grid.cells()[0][0].fg, Color::Rgb(Rgb::new(255, 0, 0)));
}

#[test]
fn set_background_applies_to_printed_cell() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::SetBackground(Color::Rgb(Rgb::new(0, 0, 255))));
    grid.apply(&TerminalCommand::Print('B'));
    assert_eq!(grid.cells()[0][0].bg, Color::Rgb(Rgb::new(0, 0, 255)));
}

#[test]
fn set_bold_flag_applies_to_printed_cell() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::SetBold);
    grid.apply(&TerminalCommand::Print('B'));
    assert!(grid.cells()[0][0].flags.contains(CellFlags::BOLD));
}

#[test]
fn multiple_flags_combine() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::SetBold);
    grid.apply(&TerminalCommand::SetItalic);
    grid.apply(&TerminalCommand::SetUnderline);
    grid.apply(&TerminalCommand::Print('X'));
    let flags = grid.cells()[0][0].flags;
    assert!(flags.contains(CellFlags::BOLD));
    assert!(flags.contains(CellFlags::ITALIC));
    assert!(flags.contains(CellFlags::UNDERLINE));
}

#[test]
fn reset_attributes_clears_all() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::SetBold);
    grid.apply(&TerminalCommand::SetForeground(Color::Indexed(1)));
    grid.apply(&TerminalCommand::ResetAttributes);
    grid.apply(&TerminalCommand::Print('N'));
    let cell = &grid.cells()[0][0];
    assert_eq!(cell.fg, Color::Default);
    assert_eq!(cell.bg, Color::Default);
    assert!(cell.flags.is_empty());
}

// === Step 5: Cursor movement ===

#[test]
fn cursor_position_one_indexed() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::CursorPosition { row: 3, col: 5 });
    grid.apply(&TerminalCommand::Print('X'));
    assert_eq!(grid.cells()[2][4].character, 'X');
}

#[test]
fn cursor_up() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::CursorPosition { row: 5, col: 1 });
    grid.apply(&TerminalCommand::CursorUp(2));
    grid.apply(&TerminalCommand::Print('U'));
    assert_eq!(grid.cells()[2][0].character, 'U');
}

#[test]
fn cursor_down() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::CursorDown(3));
    grid.apply(&TerminalCommand::Print('D'));
    assert_eq!(grid.cells()[3][0].character, 'D');
}

#[test]
fn cursor_forward() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::CursorForward(5));
    grid.apply(&TerminalCommand::Print('F'));
    assert_eq!(grid.cells()[0][5].character, 'F');
}

#[test]
fn cursor_back() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::CursorForward(5));
    grid.apply(&TerminalCommand::CursorBack(2));
    grid.apply(&TerminalCommand::Print('B'));
    assert_eq!(grid.cells()[0][3].character, 'B');
}

#[test]
fn cursor_up_clamps_at_top() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::CursorUp(100));
    grid.apply(&TerminalCommand::Print('T'));
    assert_eq!(grid.cells()[0][0].character, 'T');
}

#[test]
fn cursor_down_clamps_at_bottom() {
    let mut grid = Grid::new(80, 5);
    grid.apply(&TerminalCommand::CursorDown(100));
    grid.apply(&TerminalCommand::Print('B'));
    assert_eq!(grid.cells()[4][0].character, 'B');
}

#[test]
fn cursor_forward_clamps_at_right() {
    let mut grid = Grid::new(10, 5);
    grid.apply(&TerminalCommand::CursorForward(100));
    grid.apply(&TerminalCommand::Print('R'));
    assert_eq!(grid.cells()[0][9].character, 'R');
}

#[test]
fn cursor_back_clamps_at_left() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::CursorBack(100));
    grid.apply(&TerminalCommand::Print('L'));
    assert_eq!(grid.cells()[0][0].character, 'L');
}

#[test]
fn cursor_position_clamps_to_grid() {
    let mut grid = Grid::new(10, 5);
    grid.apply(&TerminalCommand::CursorPosition { row: 100, col: 100 });
    grid.apply(&TerminalCommand::Print('C'));
    assert_eq!(grid.cells()[4][9].character, 'C');
}

// === Step 6: Newline, CR, Backspace, Tab, Bell ===

#[test]
fn newline_moves_cursor_down() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::Print('A'));
    grid.apply(&TerminalCommand::Newline);
    grid.apply(&TerminalCommand::Print('B'));
    assert_eq!(grid.cells()[0][0].character, 'A');
    assert_eq!(grid.cells()[1][1].character, 'B');
}

#[test]
fn newline_scrolls_at_bottom() {
    let mut grid = Grid::new(5, 3);
    // Fill 3 rows (CR+LF to move to next line)
    for c in "AAAAA".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Newline);
    for c in "BBBBB".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Newline);
    for c in "CCCCC".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    // Now at bottom (row 2). Newline should scroll.
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Newline);
    grid.apply(&TerminalCommand::Print('D'));
    // Row 0 should now be what was row 1 (BBBBB)
    assert_eq!(grid.cells()[0][0].character, 'B');
    // Row 1 should now be what was row 2 (CCCCC)
    assert_eq!(grid.cells()[1][0].character, 'C');
    // Row 2 should have D at col 0
    assert_eq!(grid.cells()[2][0].character, 'D');
}

#[test]
fn carriage_return_resets_column() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::Print('A'));
    grid.apply(&TerminalCommand::Print('B'));
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Print('X'));
    assert_eq!(grid.cells()[0][0].character, 'X');
    assert_eq!(grid.cells()[0][1].character, 'B');
}

#[test]
fn backspace_moves_cursor_back_no_erase() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::Print('A'));
    grid.apply(&TerminalCommand::Print('B'));
    grid.apply(&TerminalCommand::Backspace);
    grid.apply(&TerminalCommand::Print('C'));
    // B should be overwritten with C
    assert_eq!(grid.cells()[0][0].character, 'A');
    assert_eq!(grid.cells()[0][1].character, 'C');
}

#[test]
fn backspace_clamps_at_zero() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::Backspace);
    grid.apply(&TerminalCommand::Print('Z'));
    assert_eq!(grid.cells()[0][0].character, 'Z');
}

#[test]
fn delete_chars_shifts_left_and_fills_blank() {
    let mut grid = Grid::new(80, 24);
    // Write "ABCDE"
    for c in "ABCDE".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    // Move cursor to col 1 (B)
    grid.apply(&TerminalCommand::CursorPosition { row: 1, col: 2 });
    // Delete 2 chars at cursor
    grid.apply(&TerminalCommand::DeleteChars(2));
    // B,C removed; D,E shift left
    assert_eq!(grid.cells()[0][0].character, 'A');
    assert_eq!(grid.cells()[0][1].character, 'D');
    assert_eq!(grid.cells()[0][2].character, 'E');
    assert_eq!(grid.cells()[0][3].character, ' ');
    assert_eq!(grid.cells()[0][4].character, ' ');
}

#[test]
fn delete_chars_at_end_clears() {
    let mut grid = Grid::new(80, 24);
    for c in "AB".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    // cursor at col 1
    grid.apply(&TerminalCommand::CursorPosition { row: 1, col: 2 });
    grid.apply(&TerminalCommand::DeleteChars(5));
    assert_eq!(grid.cells()[0][0].character, 'A');
    assert_eq!(grid.cells()[0][1].character, ' ');
}

#[test]
fn tab_advances_to_next_tabstop() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::Print('A'));
    grid.apply(&TerminalCommand::Tab);
    grid.apply(&TerminalCommand::Print('B'));
    // Tab from col 1 should go to col 8
    assert_eq!(grid.cells()[0][8].character, 'B');
}

#[test]
fn tab_from_tabstop_goes_to_next() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::Tab); // col 0 → col 8
    grid.apply(&TerminalCommand::Tab); // col 8 → col 16
    grid.apply(&TerminalCommand::Print('T'));
    assert_eq!(grid.cells()[0][16].character, 'T');
}

#[test]
fn bell_is_noop() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::Print('A'));
    grid.apply(&TerminalCommand::Bell);
    grid.apply(&TerminalCommand::Print('B'));
    assert_eq!(grid.cells()[0][0].character, 'A');
    assert_eq!(grid.cells()[0][1].character, 'B');
}

// === Step 7: Erase ===

#[test]
fn erase_in_line_cursor_to_end() {
    let mut grid = Grid::new(10, 1);
    for c in "ABCDEFGHIJ".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CursorPosition { row: 1, col: 4 }); // 0-indexed col 3
    grid.apply(&TerminalCommand::EraseInLine(0));
    assert_eq!(grid.cells()[0][0].character, 'A');
    assert_eq!(grid.cells()[0][1].character, 'B');
    assert_eq!(grid.cells()[0][2].character, 'C');
    assert_eq!(grid.cells()[0][3].character, ' '); // erased
    assert_eq!(grid.cells()[0][9].character, ' '); // erased
}

#[test]
fn erase_in_line_start_to_cursor() {
    let mut grid = Grid::new(10, 1);
    for c in "ABCDEFGHIJ".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CursorPosition { row: 1, col: 4 }); // 0-indexed col 3
    grid.apply(&TerminalCommand::EraseInLine(1));
    assert_eq!(grid.cells()[0][0].character, ' '); // erased
    assert_eq!(grid.cells()[0][3].character, ' '); // erased (inclusive)
    assert_eq!(grid.cells()[0][4].character, 'E');
    assert_eq!(grid.cells()[0][9].character, 'J');
}

#[test]
fn erase_in_line_entire_line() {
    let mut grid = Grid::new(10, 1);
    for c in "ABCDEFGHIJ".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CursorPosition { row: 1, col: 4 });
    grid.apply(&TerminalCommand::EraseInLine(2));
    for i in 0..10 {
        assert_eq!(grid.cells()[0][i].character, ' ');
    }
}

#[test]
fn erase_in_display_cursor_to_end() {
    let mut grid = Grid::new(5, 3);
    // Fill all
    for r in 0..3 {
        grid.apply(&TerminalCommand::CursorPosition { row: r + 1, col: 1 });
        for c in "ABCDE".chars() {
            grid.apply(&TerminalCommand::Print(c));
        }
    }
    grid.apply(&TerminalCommand::CursorPosition { row: 2, col: 3 }); // row 1, col 2
    grid.apply(&TerminalCommand::EraseInDisplay(0));
    // row 0 should be untouched
    assert_eq!(grid.cells()[0][0].character, 'A');
    // row 1, col 0-1 untouched
    assert_eq!(grid.cells()[1][0].character, 'A');
    assert_eq!(grid.cells()[1][1].character, 'B');
    // row 1, col 2+ erased
    assert_eq!(grid.cells()[1][2].character, ' ');
    // row 2 fully erased
    assert_eq!(grid.cells()[2][0].character, ' ');
}

#[test]
fn erase_in_display_start_to_cursor() {
    let mut grid = Grid::new(5, 3);
    for r in 0..3 {
        grid.apply(&TerminalCommand::CursorPosition { row: r + 1, col: 1 });
        for c in "ABCDE".chars() {
            grid.apply(&TerminalCommand::Print(c));
        }
    }
    grid.apply(&TerminalCommand::CursorPosition { row: 2, col: 3 }); // row 1, col 2
    grid.apply(&TerminalCommand::EraseInDisplay(1));
    // row 0 fully erased
    assert_eq!(grid.cells()[0][0].character, ' ');
    // row 1, col 0-2 erased (inclusive)
    assert_eq!(grid.cells()[1][0].character, ' ');
    assert_eq!(grid.cells()[1][2].character, ' ');
    // row 1, col 3+ untouched
    assert_eq!(grid.cells()[1][3].character, 'D');
    // row 2 untouched
    assert_eq!(grid.cells()[2][0].character, 'A');
}

#[test]
fn erase_in_display_entire_screen() {
    let mut grid = Grid::new(5, 3);
    for r in 0..3 {
        grid.apply(&TerminalCommand::CursorPosition { row: r + 1, col: 1 });
        for c in "ABCDE".chars() {
            grid.apply(&TerminalCommand::Print(c));
        }
    }
    grid.apply(&TerminalCommand::EraseInDisplay(2));
    for r in 0..3 {
        for c in 0..5 {
            assert_eq!(grid.cells()[r][c], Cell::default());
        }
    }
}

// === Erase preserves current background color ===

#[test]
fn erase_in_line_preserves_current_bg() {
    let mut grid = Grid::new(10, 1);
    for c in "ABCDE".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    // Set background to blue, then erase from cursor to end
    grid.apply(&TerminalCommand::SetBackground(Color::Rgb(Rgb::new(0, 0, 255))));
    grid.apply(&TerminalCommand::CursorPosition { row: 1, col: 3 }); // col 2
    grid.apply(&TerminalCommand::EraseInLine(0));
    // Erased cells should have blue background
    assert_eq!(grid.cells()[0][2].bg, Color::Rgb(Rgb::new(0, 0, 255)));
    assert_eq!(grid.cells()[0][9].bg, Color::Rgb(Rgb::new(0, 0, 255)));
    // Non-erased cells should keep original bg
    assert_eq!(grid.cells()[0][0].bg, Color::Default);
}

#[test]
fn erase_in_line_start_to_cursor_preserves_bg() {
    let mut grid = Grid::new(10, 1);
    for c in "ABCDEFGHIJ".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::SetBackground(Color::Indexed(1)));
    grid.apply(&TerminalCommand::CursorPosition { row: 1, col: 4 }); // col 3
    grid.apply(&TerminalCommand::EraseInLine(1));
    // Erased cells (0..=3) should have indexed(1) bg
    assert_eq!(grid.cells()[0][0].bg, Color::Indexed(1));
    assert_eq!(grid.cells()[0][3].bg, Color::Indexed(1));
    // Non-erased cells keep original bg
    assert_eq!(grid.cells()[0][4].bg, Color::Default);
}

#[test]
fn erase_in_display_preserves_current_bg() {
    let mut grid = Grid::new(5, 3);
    for r in 0..3 {
        grid.apply(&TerminalCommand::CursorPosition { row: r + 1, col: 1 });
        for c in "ABCDE".chars() {
            grid.apply(&TerminalCommand::Print(c));
        }
    }
    grid.apply(&TerminalCommand::SetBackground(Color::Rgb(Rgb::new(50, 50, 50))));
    grid.apply(&TerminalCommand::CursorPosition { row: 2, col: 3 }); // row 1, col 2
    grid.apply(&TerminalCommand::EraseInDisplay(0));
    // Erased cells should have the set bg
    assert_eq!(grid.cells()[1][2].bg, Color::Rgb(Rgb::new(50, 50, 50)));
    assert_eq!(grid.cells()[2][0].bg, Color::Rgb(Rgb::new(50, 50, 50)));
    // Non-erased cells keep original bg
    assert_eq!(grid.cells()[0][0].bg, Color::Default);
}

#[test]
fn erase_with_default_bg_uses_default() {
    let mut grid = Grid::new(5, 1);
    for c in "ABCDE".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    // current_bg is Default, erase should use default bg
    grid.apply(&TerminalCommand::CursorPosition { row: 1, col: 1 });
    grid.apply(&TerminalCommand::EraseInLine(0));
    assert_eq!(grid.cells()[0][0].bg, Color::Default);
}

// === Step 8: Resize ===

#[test]
fn resize_expand_fills_with_default() {
    let mut grid = Grid::new(5, 3);
    grid.apply(&TerminalCommand::Print('A'));
    grid.resize(10, 5);
    let cells = grid.cells();
    assert_eq!(cells.len(), 5);
    assert_eq!(cells[0].len(), 10);
    assert_eq!(cells[0][0].character, 'A');
    assert_eq!(cells[0][5], Cell::default());
    assert_eq!(cells[3][0], Cell::default());
}

#[test]
fn resize_shrink_truncates() {
    let mut grid = Grid::new(10, 5);
    grid.apply(&TerminalCommand::CursorPosition { row: 1, col: 8 });
    grid.apply(&TerminalCommand::Print('Z'));
    grid.resize(5, 3);
    let cells = grid.cells();
    assert_eq!(cells.len(), 3);
    assert_eq!(cells[0].len(), 5);
}

#[test]
fn resize_clamps_cursor() {
    let mut grid = Grid::new(10, 10);
    grid.apply(&TerminalCommand::CursorPosition { row: 8, col: 8 }); // row 7, col 7
    grid.resize(5, 5);
    grid.apply(&TerminalCommand::Print('C'));
    // Cursor should be clamped to (4,4)
    assert_eq!(grid.cells()[4][4].character, 'C');
}

// === Step 9: cursor_pos ===

#[test]
fn cursor_pos_initial() {
    let grid = Grid::new(80, 24);
    assert_eq!(grid.cursor_pos(), (0, 0));
}

#[test]
fn cursor_pos_after_print() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::Print('A'));
    grid.apply(&TerminalCommand::Print('B'));
    assert_eq!(grid.cursor_pos(), (0, 2));
}

#[test]
fn cursor_pos_after_cursor_position() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::CursorPosition { row: 5, col: 10 });
    assert_eq!(grid.cursor_pos(), (4, 9));
}

// === SGR individual reset codes ===

#[test]
fn reset_inverse_clears_inverse_flag() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::SetInverse);
    grid.apply(&TerminalCommand::Print('A'));
    grid.apply(&TerminalCommand::ResetInverse);
    grid.apply(&TerminalCommand::Print('B'));

    assert!(grid.cells()[0][0].flags.contains(CellFlags::INVERSE));
    assert!(!grid.cells()[0][1].flags.contains(CellFlags::INVERSE));
}

#[test]
fn reset_bold_clears_bold_and_dim() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::SetBold);
    grid.apply(&TerminalCommand::SetDim);
    grid.apply(&TerminalCommand::Print('A'));
    grid.apply(&TerminalCommand::ResetBold);
    grid.apply(&TerminalCommand::Print('B'));

    let a = grid.cells()[0][0].flags;
    assert!(a.contains(CellFlags::BOLD));
    assert!(a.contains(CellFlags::DIM));
    let b = grid.cells()[0][1].flags;
    assert!(!b.contains(CellFlags::BOLD));
    assert!(!b.contains(CellFlags::DIM));
}

#[test]
fn reset_individual_preserves_other_flags() {
    let mut grid = Grid::new(80, 24);
    grid.apply(&TerminalCommand::SetBold);
    grid.apply(&TerminalCommand::SetInverse);
    grid.apply(&TerminalCommand::ResetInverse);
    grid.apply(&TerminalCommand::Print('X'));

    let flags = grid.cells()[0][0].flags;
    assert!(flags.contains(CellFlags::BOLD));
    assert!(!flags.contains(CellFlags::INVERSE));
}

// === Scrollback buffer ===

#[test]
fn scroll_up_saves_row_to_scrollback() {
    let mut grid = Grid::new(5, 2);
    for c in "AAAAA".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Newline);
    for c in "BBBBB".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    // Newline at bottom triggers scroll_up
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Newline);
    // "AAAAA" should be in scrollback
    assert_eq!(grid.scrollback_len(), 1);
    assert_eq!(grid.cells()[0][0].character, 'B');
}

#[test]
fn scrollback_max_size_trims_oldest() {
    let mut grid = Grid::new(3, 1);
    // Each newline at bottom scrolls up, pushing the current row to scrollback
    for i in 0..(MAX_SCROLLBACK + 100) {
        let c = if i % 2 == 0 { 'A' } else { 'B' };
        grid.apply(&TerminalCommand::CarriageReturn);
        grid.apply(&TerminalCommand::Print(c));
        grid.apply(&TerminalCommand::Newline);
    }
    assert!(grid.scrollback_len() <= MAX_SCROLLBACK);
}

#[test]
fn visible_cells_at_offset_zero_returns_current() {
    let mut grid = Grid::new(5, 2);
    grid.apply(&TerminalCommand::Print('A'));
    let vis = grid.visible_cells();
    assert_eq!(vis.len(), 2);
    assert_eq!(vis[0][0].character, 'A');
}

#[test]
fn visible_cells_with_scroll_offset_shows_scrollback() {
    let mut grid = Grid::new(5, 2);
    // Row "AAAAA"
    for c in "AAAAA".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Newline);
    // Row "BBBBB"
    for c in "BBBBB".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Newline);
    // Row "CCCCC" now on screen row 0, "AAAAA" in scrollback
    for c in "CCCCC".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    // scrollback has "AAAAA", cells has ["BBBBB", "CCCCC"]
    assert_eq!(grid.scroll_offset(), 0);
    grid.scroll_up_view(1);
    assert_eq!(grid.scroll_offset(), 1);
    let vis = grid.visible_cells();
    assert_eq!(vis.len(), 2);
    // Should see "AAAAA" and "BBBBB"
    assert_eq!(vis[0][0].character, 'A');
    assert_eq!(vis[1][0].character, 'B');
}

#[test]
fn scroll_down_view_decreases_offset() {
    let mut grid = Grid::new(5, 2);
    for c in "AAAAA".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Newline);
    for c in "BBBBB".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Newline);
    grid.scroll_up_view(1);
    grid.scroll_down_view(1);
    assert_eq!(grid.scroll_offset(), 0);
}

#[test]
fn reset_scroll_sets_offset_to_zero() {
    let mut grid = Grid::new(5, 2);
    for c in "AAAAA".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Newline);
    for c in "BBBBB".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Newline);
    grid.scroll_up_view(1);
    grid.reset_scroll();
    assert_eq!(grid.scroll_offset(), 0);
}

#[test]
fn scroll_offset_nonzero_when_scrolled_up() {
    let mut grid = Grid::new(5, 2);
    for c in "AAAAA".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Newline);
    for c in "BBBBB".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Newline);
    // User scrolls up
    grid.scroll_up_view(1);
    assert!(grid.scroll_offset() > 0);
    // Simulate: caller should NOT reset_scroll when offset > 0
    // (auto-scroll only when already at bottom)
    if grid.scroll_offset() == 0 {
        grid.reset_scroll();
    }
    // Scroll position should be preserved
    assert_eq!(grid.scroll_offset(), 1);
}

#[test]
fn auto_scroll_when_already_at_bottom() {
    let mut grid = Grid::new(5, 2);
    for c in "AAAAA".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Newline);
    // Already at bottom (offset == 0)
    assert_eq!(grid.scroll_offset(), 0);
    // Simulate: caller resets scroll when at bottom
    if grid.scroll_offset() == 0 {
        grid.reset_scroll();
    }
    assert_eq!(grid.scroll_offset(), 0);
}

#[test]
fn scroll_offset_increases_when_scrollback_grows() {
    let mut grid = Grid::new(5, 2);
    // Push "AAAAA" to scrollback
    for c in "AAAAA".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Newline);
    for c in "BBBBB".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Newline);
    // User scrolls up 1
    grid.scroll_up_view(1);
    assert_eq!(grid.scroll_offset(), 1);
    let vis = grid.visible_cells();
    assert_eq!(vis[0][0].character, 'A');

    // New output causes another scroll_up (newline at bottom)
    for c in "CCCCC".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Newline);

    // scroll_offset should have increased to keep viewing the same content
    assert_eq!(grid.scroll_offset(), 2);
    let vis = grid.visible_cells();
    assert_eq!(vis[0][0].character, 'A');
}

#[test]
fn scroll_up_view_clamps_to_scrollback_len() {
    let mut grid = Grid::new(5, 2);
    for c in "AAAAA".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Newline);
    for c in "BBBBB".chars() {
        grid.apply(&TerminalCommand::Print(c));
    }
    grid.apply(&TerminalCommand::CarriageReturn);
    grid.apply(&TerminalCommand::Newline);
    // Only 1 row in scrollback
    grid.scroll_up_view(100);
    assert_eq!(grid.scroll_offset(), 1);
}

#[test]
fn cursor_visible_default_true() {
    let grid = Grid::new(10, 5);
    assert!(grid.cursor_visible());
}

#[test]
fn hide_cursor_then_show_cursor() {
    let mut grid = Grid::new(10, 5);
    grid.apply(&TerminalCommand::HideCursor);
    assert!(!grid.cursor_visible());
    grid.apply(&TerminalCommand::ShowCursor);
    assert!(grid.cursor_visible());
}
