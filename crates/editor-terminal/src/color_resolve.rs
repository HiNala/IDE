//! Resolve [`alacritty_terminal::vte::ansi::Color`] to [`Rgb`] using palette fallbacks.

use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};

/// Default foreground / background when the palette slot is unset (VS Code dark–like).
pub const DEFAULT_FG: Rgb = Rgb { r: 0xd4, g: 0xd4, b: 0xd4 };
pub const DEFAULT_BG: Rgb = Rgb { r: 0x1e, g: 0x1e, b: 0x1e };

fn dim_rgb(mut c: Rgb) -> Rgb {
    c.r /= 2;
    c.g /= 2;
    c.b /= 2;
    c
}

/// Standard xterm 256-color cube for index `i` when not overridden in [`Colors`].
fn indexed_rgb(i: u8) -> Rgb {
    if i < 16 {
        return named_ansi(i as usize);
    }
    if i < 232 {
        let i = i - 16;
        let r = i / 36 % 6;
        let g = i / 6 % 6;
        let b = i % 6;
        let step = |n: u8| -> u8 {
            if n == 0 {
                0
            } else {
                55 + 40 * (n - 1)
            }
        };
        return Rgb { r: step(r), g: step(g), b: step(b) };
    }
    let level = 10 + (i as u16 - 232) * 10;
    let v = level.min(255) as u8;
    Rgb { r: v, g: v, b: v }
}

fn named_ansi(idx: usize) -> Rgb {
    const T: [[u8; 3]; 16] = [
        [0x1e, 0x1e, 0x1e],
        [0xf4, 0x47, 0x47],
        [0x0d, 0xb3, 0x79],
        [0xff, 0xc1, 0x07],
        [0x24, 0x6e, 0xd6],
        [0xb3, 0x9c, 0xef],
        [0x11, 0xa8, 0xcd],
        [0xe5, 0xe5, 0xe5],
        [0x66, 0x66, 0x66],
        [0xf4, 0x47, 0x47],
        [0x0d, 0xb3, 0x79],
        [0xff, 0xc1, 0x07],
        [0x24, 0x6e, 0xd6],
        [0xb3, 0x9c, 0xef],
        [0x11, 0xa8, 0xcd],
        [0xff, 0xff, 0xff],
    ];
    let t = T[idx.min(15)];
    Rgb { r: t[0], g: t[1], b: t[2] }
}

fn named_fallback(nc: NamedColor, fg: Rgb, bg: Rgb) -> Rgb {
    match nc {
        NamedColor::Foreground => fg,
        NamedColor::Background => bg,
        NamedColor::Cursor | NamedColor::BrightForeground => fg,
        _ => {
            let idx = nc as usize;
            if idx < 16 {
                named_ansi(idx)
            } else {
                fg
            }
        }
    }
}

/// Resolve a cell color to RGB using dynamic palette entries when present.
pub fn resolve_color(c: Color, palette: &Colors, fg: Rgb, bg: Rgb) -> Rgb {
    match c {
        Color::Named(nc) => palette[nc].unwrap_or_else(|| named_fallback(nc, fg, bg)),
        Color::Spec(rgb) => rgb,
        Color::Indexed(i) => palette[i as usize].unwrap_or_else(|| indexed_rgb(i)),
    }
}

use alacritty_terminal::term::cell::{Cell, Flags};

/// Foreground / background for a [`Cell`], honoring inverse / dim flags.
pub fn resolve_cell_colors(cell: &Cell, palette: &Colors, fg: Rgb, bg: Rgb) -> (Rgb, Rgb) {
    let mut f = resolve_color(cell.fg, palette, fg, bg);
    let mut b = resolve_color(cell.bg, palette, fg, bg);
    if cell.flags.contains(Flags::INVERSE) {
        std::mem::swap(&mut f, &mut b);
    }
    if cell.flags.contains(Flags::DIM) && !cell.flags.contains(Flags::INVERSE) {
        f = dim_rgb(f);
    }
    (f, b)
}
