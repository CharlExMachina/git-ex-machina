//! The app's design tokens: every colour, shadow, radius and type style it draws comes from
//! here, by name (the names follow CSS custom properties: `--accent`, `--shadow-raised`). New UI
//! uses these names; a new look starts as a new token here, never a literal in a view.

use rune::{Color, Shadow, TextStyle};

/// The colour tokens, dark or light.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub dark: bool,
    pub chrome: Color,
    pub canvas: Color,
    pub raised: Color,
    pub control: Color,
    /// Where a control's gradient ends: `linear-gradient(var(--control), var(--control-bottom))`.
    pub control_bottom: Color,
    pub control_hover: Color,
    pub well: Color,
    pub line: Color,
    /// `--edge`: a control's outline (the Viewed checkbox).
    pub edge: Color,
    pub ink: Color,
    pub ink_muted: Color,
    pub ink_faint: Color,
    pub accent: Color,
    pub accent_top: Color,
    pub accent_soft: Color,
    pub on_accent: Color,
    pub focus: Color,
    pub added: Color,
    pub added_soft: Color,
    pub removed: Color,
    pub removed_soft: Color,
    /// `--lane-1` … `--lane-6`.
    pub lanes: [Color; 6],
    pub on_lane: Color,
    /// `--scrim`: what dims the window behind a sheet.
    pub scrim: Color,
    shadows: Shadows,
}

/// `0xRRGGBB` in sRGB to Rune's linear colour.
pub fn rgb(v: u32) -> Color {
    rgba(v, 1.0)
}

/// `rgba(r, g, b, a)` in sRGB to Rune's linear colour.
pub fn rgba(v: u32, a: f32) -> Color {
    let ch = |shift: u32| rune::color::srgb_decode(((v >> shift) & 0xff) as f32 / 255.0);
    Color { r: ch(16), g: ch(8), b: ch(0), a }
}

/// `color-mix(in srgb, <fg> <a>, transparent)` laid over `bg`, as the browser blends it (in
/// sRGB), as one opaque colour: `fg` at `a` over `bg`.
pub fn mix(fg: Color, a: f32, bg: Color) -> Color {
    use rune::color::{srgb_decode, srgb_encode};
    let m = |f: f32, b: f32| srgb_decode(srgb_encode(f) * a + srgb_encode(b) * (1.0 - a));
    Color { r: m(fg.r, bg.r), g: m(fg.g, bg.g), b: m(fg.b, bg.b), a: 1.0 }
}

/// The shadow tokens. `pressed`, `lifted` and `well` keep their colours only (their geometry
/// is the same in both themes); `raised`, `primary` and `badge` are whole. The `shadow_*`
/// methods list each token's layers bottom first, because Rune paints later shadows on top
/// (CSS lists the top one first).
#[derive(Clone, Copy, Debug)]
struct Shadows {
    /// `--shadow-raised`, whole, bottom layer first: its layers differ by theme (light adds a
    /// second, softer drop), so this one keeps its geometry too.
    raised: [Shadow; 4],
    /// `--shadow-primary`: the primary button's bevel, edge and drop, bottom layer first.
    primary: [Shadow; 5],
    /// `--shadow-badge`: a badge on the primary button, set into it.
    badge: Shadow,
    /// `--shadow-pressed`: inner shade, edge ring.
    pressed: [Color; 2],
    /// `--shadow-lifted`: top highlight, edge ring, drop.
    lifted: [Color; 3],
    /// `--shadow-well`: inner shade, inner ring.
    well: [Color; 2],
    /// `--shadow-overlay`: top highlight, edge ring, a near drop and a far one.
    overlay: [Color; 4],
}

impl Theme {
    pub fn new(dark: bool) -> Theme {
        if dark { Theme::dark() } else { Theme::light() }
    }

    fn dark() -> Theme {
        Theme {
            dark: true,
            chrome: rgb(0x121110),
            canvas: rgb(0x191817),
            raised: rgb(0x201e1c),
            control: rgb(0x2a2725),
            control_bottom: rgb(0x2a2725),
            control_hover: rgb(0x322f2c),
            well: rgb(0x0e0d0c),
            line: rgb(0x2b2826),
            edge: rgb(0x7a736b),
            ink: rgb(0xf1ede8),
            ink_muted: rgb(0xa8a199),
            ink_faint: rgb(0x7d766f),
            accent: rgb(0xe2ad62),
            accent_top: rgb(0xecbe7c),
            accent_soft: rgb(0x3a2d1b),
            on_accent: rgb(0x1c1307),
            focus: rgb(0xeab872),
            added: rgb(0x7ccfa8),
            added_soft: rgb(0x1c2e25),
            removed: rgb(0xec7e6b),
            removed_soft: rgb(0x35201c),
            lanes: [rgb(0xe2ad62), rgb(0x6fc2b4), rgb(0x86ade8), rgb(0xe88fa4), rgb(0xb3c56e), rgb(0xbba4e8)],
            on_lane: rgb(0x121110),
            scrim: rgba(0x000000, 0.5),
            shadows: Shadows {
                // inset 0 1px 0 rgba(255,255,255,0.07), 0 0 0 1px rgba(0,0,0,0.55), 0 1px 2px rgba(0,0,0,0.5)
                raised: [
                    NONE,
                    outer(0.0, 1.0, 2.0, 0.0, rgba(0x000000, 0.5)),
                    outer(0.0, 0.0, 0.0, 1.0, rgba(0x000000, 0.55)),
                    inset(0.0, 1.0, 0.0, 0.0, rgba(0xffffff, 0.07)),
                ],
                // inset 0 1px 0 rgba(255,246,225,0.6), inset 0 -1px 0 rgba(90,50,0,0.3),
                // 0 0 0 1px rgba(0,0,0,0.55), 0 1px 2px rgba(0,0,0,0.5)
                primary: [
                    NONE,
                    outer(0.0, 1.0, 2.0, 0.0, rgba(0x000000, 0.5)),
                    outer(0.0, 0.0, 0.0, 1.0, rgba(0x000000, 0.55)),
                    inset(0.0, -1.0, 0.0, 0.0, rgba(0x5a3200, 0.3)),
                    inset(0.0, 1.0, 0.0, 0.0, rgba(0xfff6e1, 0.6)),
                ],
                // inset 0 1px 1px rgba(0,0,0,0.6)
                badge: inset(0.0, 1.0, 1.0, 0.0, rgba(0x000000, 0.6)),
                pressed: [rgba(0x000000, 0.6), rgba(0x000000, 0.55)],
                lifted: [rgba(0xffffff, 0.06), rgba(0x000000, 0.45), rgba(0x000000, 0.35)],
                well: [rgba(0x000000, 0.5), rgba(0xffffff, 0.03)],
                overlay: [rgba(0xffffff, 0.07), rgba(0x000000, 0.6), rgba(0x000000, 0.4), rgba(0x000000, 0.45)],
            },
        }
    }

    fn light() -> Theme {
        Theme {
            dark: false,
            chrome: rgb(0xeceae6),
            canvas: rgb(0xf7f5f2),
            raised: rgb(0xffffff),
            control: rgb(0xffffff),
            control_bottom: rgb(0xfaf8f5),
            control_hover: rgb(0xf3f0ec),
            well: rgb(0xe6e2dc),
            line: rgb(0xe2ddd6),
            edge: rgb(0x877f75),
            ink: rgb(0x1d1b19),
            ink_muted: rgb(0x665f57),
            ink_faint: rgb(0x8a8278),
            accent: rgb(0x8f5a12),
            accent_top: rgb(0xb07628),
            accent_soft: rgb(0xf4e5ca),
            on_accent: rgb(0xffffff),
            focus: rgb(0x8a560f),
            added: rgb(0x1d7650),
            added_soft: rgb(0xe3f2e9),
            removed: rgb(0xb23d28),
            removed_soft: rgb(0xf9e4df),
            lanes: [rgb(0x8f5c17), rgb(0x1f7768), rgb(0x2f62b0), rgb(0xb03d5b), rgb(0x5b6f1c), rgb(0x6c4fb5)],
            on_lane: rgb(0xffffff),
            scrim: rgba(0x2e2214, 0.22),
            shadows: Shadows {
                // inset 0 1px 0 #ffffff, 0 0 0 1px rgba(46,34,20,0.15), 0 1px 1px rgba(46,34,20,0.12),
                // 0 2px 4px -2px rgba(46,34,20,0.22)
                raised: [
                    outer(0.0, 2.0, 4.0, -2.0, rgba(0x2e2214, 0.22)),
                    outer(0.0, 1.0, 1.0, 0.0, rgba(0x2e2214, 0.12)),
                    outer(0.0, 0.0, 0.0, 1.0, rgba(0x2e2214, 0.15)),
                    inset(0.0, 1.0, 0.0, 0.0, rgb(0xffffff)),
                ],
                // inset 0 1px 0 rgba(255,232,190,0.38), inset 0 -1px 0 rgba(60,30,0,0.22),
                // 0 0 0 1px #774a0c, 0 1px 1px rgba(46,34,20,0.20), 0 2px 4px -2px rgba(46,34,20,0.34)
                primary: [
                    outer(0.0, 2.0, 4.0, -2.0, rgba(0x2e2214, 0.34)),
                    outer(0.0, 1.0, 1.0, 0.0, rgba(0x2e2214, 0.20)),
                    outer(0.0, 0.0, 0.0, 1.0, rgb(0x774a0c)),
                    inset(0.0, -1.0, 0.0, 0.0, rgba(0x3c1e00, 0.22)),
                    inset(0.0, 1.0, 0.0, 0.0, rgba(0xffe8be, 0.38)),
                ],
                // inset 0 1px 1px rgba(46,34,20,0.28)
                badge: inset(0.0, 1.0, 1.0, 0.0, rgba(0x2e2214, 0.28)),
                pressed: [rgba(0x2e2214, 0.18), rgba(0x2e2214, 0.15)],
                lifted: [rgba(0xffffff, 1.0), rgba(0x2e2214, 0.08), rgba(0x2e2214, 0.08)],
                well: [rgba(0x2e2214, 0.10), rgba(0x2e2214, 0.06)],
                overlay: [rgb(0xffffff), rgba(0x2e2214, 0.14), rgba(0x2e2214, 0.12), rgba(0x2e2214, 0.2)],
            },
        }
    }

    /// `--modified`.
    pub fn modified(&self) -> Color {
        self.accent
    }

    /// Lane `n`'s colour, 1-based: `--lane-((n - 1) % 6 + 1)`.
    pub fn lane(&self, n: usize) -> Color {
        self.lanes[(n.max(1) - 1) % 6]
    }

    /// `--shadow-raised`.
    pub fn shadow_raised(&self) -> [Shadow; 4] {
        self.shadows.raised
    }

    /// `--shadow-primary`.
    pub fn shadow_primary(&self) -> [Shadow; 5] {
        self.shadows.primary
    }

    /// `--shadow-badge`.
    pub fn shadow_badge(&self) -> Shadow {
        self.shadows.badge
    }

    /// `inset 0 1px 2px <shade>, 0 0 0 1px <ring>`.
    pub fn shadow_pressed(&self) -> [Shadow; 2] {
        let [shade, ring] = self.shadows.pressed;
        [outer(0.0, 0.0, 0.0, 1.0, ring), inset(0.0, 1.0, 2.0, 0.0, shade)]
    }

    /// `inset 0 1px 0 <highlight>, 0 0 0 1px <ring>, 0 2px 6px <drop>`.
    pub fn shadow_lifted(&self) -> [Shadow; 3] {
        let [hi, ring, drop] = self.shadows.lifted;
        [outer(0.0, 2.0, 6.0, 0.0, drop), outer(0.0, 0.0, 0.0, 1.0, ring), inset(0.0, 1.0, 0.0, 0.0, hi)]
    }

    /// `--shadow-lifted-first`: the first of neighbouring selected rows. Its top and side edges
    /// and the lit top line, no bottom edge or drop (the next row continues it).
    pub fn shadow_lifted_first(&self) -> [Shadow; 4] {
        let [hi, ring, _] = self.shadows.lifted;
        [
            outer(-1.0, 0.0, 0.0, 0.0, ring),
            outer(1.0, 0.0, 0.0, 0.0, ring),
            outer(0.0, -1.0, 0.0, 0.0, ring),
            inset(0.0, 1.0, 0.0, 0.0, hi),
        ]
    }

    /// `--shadow-lifted-middle`: a selected row with selected rows on both sides. Side edges only.
    pub fn shadow_lifted_middle(&self) -> [Shadow; 2] {
        let [_, ring, _] = self.shadows.lifted;
        [outer(-1.0, 0.0, 0.0, 0.0, ring), outer(1.0, 0.0, 0.0, 0.0, ring)]
    }

    /// `--shadow-lifted-last`: the last of neighbouring selected rows. Side and bottom edges,
    /// and the drop.
    pub fn shadow_lifted_last(&self) -> [Shadow; 4] {
        let [_, ring, drop] = self.shadows.lifted;
        [
            outer(0.0, 2.0, 6.0, 0.0, drop),
            outer(-1.0, 0.0, 0.0, 0.0, ring),
            outer(1.0, 0.0, 0.0, 0.0, ring),
            outer(0.0, 1.0, 0.0, 0.0, ring),
        ]
    }

    /// `inset 0 1px 2px <shade>, inset 0 0 0 1px <ring>`.
    pub fn shadow_well(&self) -> [Shadow; 2] {
        let [shade, ring] = self.shadows.well;
        [inset(0.0, 0.0, 0.0, 1.0, ring), inset(0.0, 1.0, 2.0, 0.0, shade)]
    }

    /// `inset 0 1px 0 <highlight>, 0 0 0 1px <ring>, 0 2px 4px <near>, 0 16px 40px <far>`: a
    /// sheet floating over the window.
    pub fn shadow_overlay(&self) -> [Shadow; 4] {
        let [hi, ring, near, far] = self.shadows.overlay;
        [
            outer(0.0, 16.0, 40.0, 0.0, far),
            outer(0.0, 2.0, 4.0, 0.0, near),
            outer(0.0, 0.0, 0.0, 1.0, ring),
            inset(0.0, 1.0, 0.0, 0.0, hi),
        ]
    }

    /// `inset 0 0 0 <width> <color>`: a ring inside the box that takes no layout space.
    pub fn ring(width: f32, color: Color) -> Shadow {
        inset(0.0, 0.0, 0.0, width, color)
    }
}

/// A slot a theme leaves empty, so both themes' tokens have the same number of layers.
const NONE: Shadow =
    Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 0.0, color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }, inset: false };

fn outer(x: f32, y: f32, blur: f32, spread: f32, color: Color) -> Shadow {
    Shadow { x, y, blur, spread, color, inset: false }
}

fn inset(x: f32, y: f32, blur: f32, spread: f32, color: Color) -> Shadow {
    Shadow { x, y, blur, spread, color, inset: true }
}

/// `--space-*`, `--radius-*` and `--size-*`.
pub mod size {
    pub const RADIUS_XS: f32 = 4.0;
    pub const RADIUS_SM: f32 = 6.0;
    pub const RADIUS_MD: f32 = 8.0;
    pub const RADIUS_LG: f32 = 12.0;
    pub const PILL: f32 = 999.0;
    pub const SIDEBAR: f32 = 248.0;
    pub const SHEET: f32 = 460.0;
    pub const INSPECTOR: f32 = 360.0;
    pub const TOOLBAR: f32 = 48.0;
    pub const ROW_REPO: f32 = 36.0;
    pub const ROW_COMMIT: f32 = 30.0;
    pub const ROW_FILE: f32 = 28.0;
}

pub const SANS: &str = "Hanken Grotesk";
pub const MONO: &str = "Geist Mono";

/// The app's fonts, bundled so every machine renders the same glyphs.
pub const FONTS: &[&[u8]] = &[
    include_bytes!("../assets/fonts/HankenGrotesk-Variable.ttf"),
    include_bytes!("../assets/fonts/GeistMono-Variable.ttf"),
];

/// The type styles, by role.
pub mod ty {
    use super::TextStyle;

    const fn sans(size: f32, weight: f32, lh: f32) -> TextStyle {
        TextStyle::new(size, weight).lh(lh)
    }
    const fn mono(size: f32, weight: f32, lh: f32) -> TextStyle {
        TextStyle::new(size, weight).mono().lh(lh)
    }

    /// `body`: 400 13px/18px.
    pub const BODY: TextStyle = sans(13.0, 400.0, 18.0);
    /// `--text-body-strong`: 500 13px/18px.
    pub const BODY_STRONG: TextStyle = sans(13.0, 500.0, 18.0);
    /// 400 12px on the app's 18px line, which every small text shares.
    pub const CAPTION: TextStyle = sans(12.0, 400.0, 18.0);
    /// 500 12px/16px: the branch in the summary, the inspector's author.
    pub const CAPTION_STRONG: TextStyle = sans(12.0, 500.0, 18.0);
    /// 500 12px (buttons and segments at `sm`).
    pub const CONTROL: TextStyle = sans(12.0, 500.0, 16.0);
    /// `.label`: 600 11px/14px, 0.06em tracking, set in capitals.
    pub const LABEL: TextStyle = sans(11.0, 600.0, 14.0).tracking(0.66);
    /// The repository row's name: 500 13px/16px.
    pub const REPO_NAME: TextStyle = sans(13.0, 500.0, 16.0);
    /// The repository row's meta line: 11.5px/14px.
    pub const REPO_META: TextStyle = sans(11.5, 400.0, 14.0);
    /// `.gu-ref`: 500 11.5px/20px.
    pub const REF: TextStyle = sans(11.5, 500.0, 20.0);
    /// `.gu-ref-prefix`: 400 11.5px/20px.
    pub const REF_PREFIX: TextStyle = sans(11.5, 400.0, 20.0);
    /// 11.5px hints (`⌘-click or ⇧-click …`).
    pub const HINT: TextStyle = sans(11.5, 400.0, 18.0);
    /// 11px column headers.
    pub const HEADER: TextStyle = sans(11.0, 400.0, 18.0);
    /// `.gu-avatar`: 600 9px.
    pub const AVATAR: TextStyle = sans(9.0, 600.0, 16.0);
    /// `.gu-repo-tile`: 600 12px.
    pub const TILE: TextStyle = sans(12.0, 600.0, 16.0);
    /// `--text-mono`: 400 12px/18px.
    pub const MONO: TextStyle = mono(12.0, 400.0, 18.0);
    /// `--text-mono-sm`: 500 11px/16px.
    pub const MONO_SM: TextStyle = mono(11.0, 500.0, 16.0);
    /// 400 11px mono (commit ids, counts, stats, kbd).
    pub const MONO_11: TextStyle = mono(11.0, 400.0, 16.0);
    /// `.gu-status`: 500 10px/16px mono.
    pub const STATUS: TextStyle = mono(10.0, 500.0, 16.0);
    /// A sheet's title: 600 15px/20px, -0.01em.
    pub const TITLE: TextStyle = sans(15.0, 600.0, 20.0).tracking(-0.15);
    /// `.display-sm`: the inspector's title, 600 18px/24px, -0.01em.
    pub const DISPLAY_SM: TextStyle = sans(18.0, 600.0, 24.0).tracking(-0.18);
    /// 600 13px: a pull request's number, the merge box's headings.
    pub const BODY_SEMI: TextStyle = sans(13.0, 600.0, 18.0);
    /// 400 11px on the body's 18px line: the review cockpit's comment counts.
    pub const SMALL: TextStyle = sans(11.0, 400.0, 18.0);
    /// `.state`: a pull request's state pill, 600 11.5px/20px.
    pub const STATE: TextStyle = sans(11.5, 600.0, 20.0);
    /// 400 12.5px/18px: a pull request's description.
    pub const DESCRIPTION: TextStyle = sans(12.5, 400.0, 18.0);
    /// `.count` in a tab: 400 10.5px/14px mono.
    pub const TAB_COUNT: TextStyle = mono(10.5, 400.0, 14.0);
    /// 500 12px/18px mono: a name in a review thread (threads sit in the hunk's mono).
    pub const MONO_STRONG: TextStyle = mono(12.0, 500.0, 18.0);
}
