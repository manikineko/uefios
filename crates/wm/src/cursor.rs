//! Arrow mouse cursor bitmap.

use crate::surface::Surface;

/// Classic arrow pointer, 12x19. ' ' = transparent, '#' = outline, '.' = fill.
const ARROW: [&str; 19] = [
    "#           ",
    "##          ",
    "#.#         ",
    "#..#        ",
    "#...#       ",
    "#....#      ",
    "#.....#     ",
    "#......#    ",
    "#.......#   ",
    "#........#  ",
    "#.....##### ",
    "#..#..#     ",
    "#.# #..#    ",
    "##  #..#    ",
    "#    #..#   ",
    "      #..#  ",
    "      #..#  ",
    "       ##   ",
    "           ",
];

pub const CURSOR_W: usize = 12;
pub const CURSOR_H: usize = 19;

pub fn draw_cursor(s: &mut Surface, x: i32, y: i32) {
    for (row, line) in ARROW.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            let px = match ch {
                '#' => Some(crate::surface::BLACK),
                '.' => Some(crate::surface::WHITE),
                _ => None,
            };
            if let Some(c) = px {
                let (dx, dy) = (x + col as i32, y + row as i32);
                if dx >= 0 && dy >= 0 {
                    s.put(dx as usize, dy as usize, c);
                }
            }
        }
    }
}
