//! Minimal stroke font for dimension text rendering.
//!
//! Supports digits 0-9, decimal point '.', and minus sign '-'.
//! Each glyph is defined as line segments in a 4×6 unit cell.
//!
//! Coordinate system: x increases right, y increases downward (0 = top of cell).
//!
//! `text_segments` converts a string to world-space line segment pairs given a
//! centre point, direction vector, up vector, and height in world units.

// ---------------------------------------------------------------------------
// 7-segment glyph geometry  (x1, y1, x2, y2) in a 4-wide × 6-tall cell
// ---------------------------------------------------------------------------

const TOP: (f32, f32, f32, f32) = (0.5, 0.0, 3.5, 0.0); // a – horizontal top
const TR:  (f32, f32, f32, f32) = (4.0, 0.5, 4.0, 2.5); // b – upper-right vertical
const BR:  (f32, f32, f32, f32) = (4.0, 3.5, 4.0, 5.5); // c – lower-right vertical
const BOT: (f32, f32, f32, f32) = (0.5, 6.0, 3.5, 6.0); // d – horizontal bottom
const BL:  (f32, f32, f32, f32) = (0.0, 3.5, 0.0, 5.5); // e – lower-left vertical
const TL:  (f32, f32, f32, f32) = (0.0, 0.5, 0.0, 2.5); // f – upper-left vertical
const MID: (f32, f32, f32, f32) = (0.5, 3.0, 3.5, 3.0); // g – horizontal middle

// Decimal point: small cross at bottom-centre of cell
const DOT_H: (f32, f32, f32, f32) = (1.5, 5.5, 2.5, 5.5);
const DOT_V: (f32, f32, f32, f32) = (2.0, 5.0, 2.0, 6.0);

// ---------------------------------------------------------------------------
// Glyph tables
// ---------------------------------------------------------------------------

fn glyph_segs(ch: char) -> &'static [(f32, f32, f32, f32)] {
    match ch {
        '0' => &[TOP, TL, TR, BL, BR, BOT],
        '1' => &[TR, BR],
        '2' => &[TOP, TR, MID, BL, BOT],
        '3' => &[TOP, TR, MID, BR, BOT],
        '4' => &[TL, TR, MID, BR],
        '5' => &[TOP, TL, MID, BR, BOT],
        '6' => &[TOP, TL, MID, BL, BR, BOT],
        '7' => &[TOP, TR, BR],
        '8' => &[TOP, TL, TR, MID, BL, BR, BOT],
        '9' => &[TOP, TL, TR, MID, BR, BOT],
        '-' => &[MID],
        '.' => &[DOT_H, DOT_V],
        _   => &[],
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Character advance: glyph width (4) + inter-character gap (1).
const ADVANCE: f32 = 5.0;

/// Build world-space line segment pairs that render `text` as stroke glyphs.
///
/// # Parameters
/// - `center`  – World-space centre of the text block `[x, y]`.
/// - `dir`     – Normalised direction along the text baseline (left → right).
/// - `up`      – Normalised direction that maps to *up* in screen space.
///               (Cell y = 0 is the top of the glyph; it maps to +`up`.)
/// - `height`  – Desired glyph height in world units (cell height = 6 units).
///
/// Returns a `Vec` of `([f32;2], [f32;2])` endpoint pairs suitable for
/// wgpu `LineList` or egui `Painter::line_segment` calls.
pub fn text_segments(
    text: &str,
    center: [f32; 2],
    dir:    [f32; 2],
    up:     [f32; 2],
    height: f32,
) -> Vec<([f32; 2], [f32; 2])> {
    let n = text.chars().count();
    if n == 0 {
        return Vec::new();
    }

    // Width of the full text block (last char has no trailing gap).
    let total_width = n as f32 * ADVANCE - 1.0;
    let scale = height / 6.0;

    let mut segs = Vec::new();

    for (i, ch) in text.chars().enumerate() {
        let char_origin_x = i as f32 * ADVANCE;

        for &(x1, y1, x2, y2) in glyph_segs(ch) {
            // Local coords relative to the block's top-left corner
            let lx1 = char_origin_x + x1;
            let ly1 = y1;
            let lx2 = char_origin_x + x2;
            let ly2 = y2;

            // Centre the block: shift so (total_width/2, 3.0) maps to `center`
            let dx1 = lx1 - total_width / 2.0;
            let dy1 = ly1 - 3.0; // cell mid-height
            let dx2 = lx2 - total_width / 2.0;
            let dy2 = ly2 - 3.0;

            // World coords:
            //   horizontal component → along `dir`
            //   vertical component   → along `up` (cell y increases downward → subtract)
            let p1 = [
                center[0] + dir[0] * dx1 * scale - up[0] * dy1 * scale,
                center[1] + dir[1] * dx1 * scale - up[1] * dy1 * scale,
            ];
            let p2 = [
                center[0] + dir[0] * dx2 * scale - up[0] * dy2 * scale,
                center[1] + dir[1] * dx2 * scale - up[1] * dy2 * scale,
            ];

            segs.push((p1, p2));
        }
    }

    segs
}
