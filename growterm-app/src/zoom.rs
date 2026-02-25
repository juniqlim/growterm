const MIN_FONT_SIZE: f32 = 8.0;
const MAX_FONT_SIZE: f32 = 72.0;
const ZOOM_STEP: f32 = 2.0;

pub fn apply_zoom(current: f32, delta: f32) -> f32 {
    (current + delta).clamp(MIN_FONT_SIZE, MAX_FONT_SIZE)
}

pub fn calc_grid_size(width: u32, height: u32, cell_w: f32, cell_h: f32) -> (u16, u16) {
    let cols = (width as f32 / cell_w).floor() as u16;
    let rows = (height as f32 / cell_h).floor() as u16;
    (cols.max(1), rows.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_zoom_increases() {
        assert_eq!(apply_zoom(24.0, 2.0), 26.0);
    }

    #[test]
    fn apply_zoom_decreases() {
        assert_eq!(apply_zoom(24.0, -2.0), 22.0);
    }

    #[test]
    fn apply_zoom_clamps_at_min() {
        assert_eq!(apply_zoom(8.0, -2.0), MIN_FONT_SIZE);
    }

    #[test]
    fn apply_zoom_clamps_at_max() {
        assert_eq!(apply_zoom(72.0, 2.0), MAX_FONT_SIZE);
    }

    #[test]
    fn apply_zoom_below_min_clamps() {
        assert_eq!(apply_zoom(2.0, -10.0), MIN_FONT_SIZE);
    }

    #[test]
    fn apply_zoom_above_max_clamps() {
        assert_eq!(apply_zoom(100.0, 10.0), MAX_FONT_SIZE);
    }

    // calc_grid_size tests

    #[test]
    fn calc_grid_size_basic() {
        // 800x600 window, 10x20 cells → 80 cols, 30 rows
        assert_eq!(calc_grid_size(800, 600, 10.0, 20.0), (80, 30));
    }

    #[test]
    fn calc_grid_size_floors_partial_cells() {
        // 805x610 → floor(80.5)=80, floor(30.5)=30
        assert_eq!(calc_grid_size(805, 610, 10.0, 20.0), (80, 30));
    }

    #[test]
    fn calc_grid_size_minimum_one() {
        // Very small window → at least 1x1
        assert_eq!(calc_grid_size(1, 1, 100.0, 100.0), (1, 1));
    }
}
