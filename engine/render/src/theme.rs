//! The two palettes of styles.ts (`:root` and its `prefers-color-scheme: dark` override), as colours.
use tiny_skia::Color;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scheme {
    #[default]
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Theme {
    pub scheme: Scheme,
}

const LIGHT: &[(&str, &str)] = &[
    ("bg", "#ffffff"), ("fg", "#1b1d21"), ("fg2", "#545a63"), ("line", "#e2e5ea"), ("card", "#ffffff"), ("well", "#f4f5f7"),
    ("primary", "#2f5bea"), ("primary-fg", "#ffffff"), ("primary-soft", "#e7ecfd"), ("on-tone", "#ffffff"),
    ("success", "#156e3b"), ("success-soft", "#e1f4e8"),
    ("danger", "#c62a2a"), ("danger-soft", "#fbe5e5"),
    ("warning", "#8a5900"), ("warning-soft", "#fdf1d6"),
    ("info", "#0b7285"), ("info-soft", "#dff5f9"),
    ("muted", "#5d646e"), ("muted-soft", "#eceef1"),
    ("white", "#ffffff"),
];

const DARK: &[(&str, &str)] = &[
    ("bg", "#121417"), ("fg", "#e8eaee"), ("fg2", "#a3a9b4"), ("line", "#2b3038"), ("card", "#1a1d22"), ("well", "#1f232a"),
    ("primary", "#7a95ff"), ("primary-fg", "#0d1326"), ("primary-soft", "#232c4d"), ("on-tone", "#121417"),
    ("success", "#5fcf8a"), ("success-soft", "#173224"),
    ("danger", "#ff7b7b"), ("danger-soft", "#3d1c1c"),
    ("warning", "#f0bb4c"), ("warning-soft", "#3b2e12"),
    ("info", "#5fd3e8"), ("info-soft", "#113038"),
    ("muted", "#9aa1ab"), ("muted-soft", "#272b33"),
    ("white", "#ffffff"),
];

pub fn hex(h: &str) -> Color {
    let v = u32::from_str_radix(h.trim_start_matches('#'), 16).unwrap_or(0);
    Color::from_rgba8(((v >> 16) & 255) as u8, ((v >> 8) & 255) as u8, (v & 255) as u8, 255)
}

impl Theme {
    pub fn light() -> Theme {
        Theme { scheme: Scheme::Light }
    }
    pub fn dark() -> Theme {
        Theme { scheme: Scheme::Dark }
    }
    pub fn color(&self, key: &str) -> Color {
        let table = match self.scheme {
            Scheme::Light => LIGHT,
            Scheme::Dark => DARK,
        };
        match table.iter().find(|(k, _)| *k == key) {
            Some((_, h)) => hex(h),
            None => hex(if self.scheme == Scheme::Dark { "#e8eaee" } else { "#1b1d21" }),
        }
    }
}

fn lum(c: Color) -> f64 {
    let f = |v: f32| -> f64 {
        let v = v as f64;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * f(c.red()) + 0.7152 * f(c.green()) + 0.0722 * f(c.blue())
}

/// WCAG contrast ratio between two opaque colours.
pub fn contrast(a: Color, b: Color) -> f64 {
    let (la, lb) = (lum(a), lum(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// `top` with alpha over an opaque `under`.
pub fn over(top: Color, alpha: f32, under: Color) -> Color {
    let a = top.alpha() * alpha;
    let ch = |t: f32, u: f32| t * a + u * (1.0 - a);
    Color::from_rgba(ch(top.red(), under.red()), ch(top.green(), under.green()), ch(top.blue(), under.blue()), 1.0).unwrap()
}
