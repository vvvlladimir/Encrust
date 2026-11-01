//! The window's two palettes, dark and light; `theme::colors` picks the one in force.
#![expect(clippy::disallowed_methods, reason = "the palette is defined here")]

use egui::Color32;

/// Every colour the window is allowed to paint with.
///
/// Graphite surfaces, one ember accent, and colours kept for state. See
/// `docs/design/ui-design-system.md`.
pub struct Palette {
    /// Viewport backdrop and any trough a control is sunk into.
    pub sunken: Color32,
    /// The logo's own background: window chrome, the top bar, the rail and the layer strip.
    pub base: Color32,
    /// Inspector, tool rail and floating cards.
    pub panel: Color32,
    /// Inputs, unpressed buttons, list rows.
    pub raised: Color32,
    pub hover: Color32,
    /// A pressed control, and the track of a switch that is off.
    pub active: Color32,
    /// Section dividers and panel edges.
    pub hairline: Color32,
    /// Input borders and the base of a focus ring.
    pub line: Color32,
    /// Values and headings.
    pub text_high: Color32,
    /// Field labels.
    pub text_mid: Color32,
    /// Units and hints, never body text.
    pub text_low: Color32,
    /// Primary action, selection, active tool.
    pub accent: Color32,
    /// Pressed state and the far stop of the accent gradient.
    pub accent_deep: Color32,
    /// The accent as text or a glyph on a dark surface: the open tool, the active segment.
    pub accent_soft: Color32,
    /// Selected row background and active segment.
    pub accent_wash: Color32,
    /// Text drawn on an accent fill.
    pub on_accent: Color32,
    /// A machine that answered the last scan, and anything else reported good.
    pub ok: Color32,
    /// Mesh repair notices, out-of-volume.
    pub warn: Color32,
    /// Non-orientable mesh, write failure.
    pub danger: Color32,
    /// The washes behind a verdict a print fails on, only might, or passes, and an armed
    /// control: 14 % of each, 12 % of the danger so its own text still reads at AA.
    pub danger_wash: Color32,
    pub warn_wash: Color32,
    pub ok_wash: Color32,
    /// Transform field letters, in the CAD convention X red, Y green, Z blue.
    pub axis: [Color32; 3],
    /// A picked model: the part of a support the pointer has hold of, the row of a
    /// selected model, and the tally that counts them.
    pub picked: Color32,
    /// The wash behind a selected row, 10 % of the accent.
    pub picked_wash: Color32,
    /// The face of a handle: a switch's knob, the layer thumbs, the gizmo's circle. Light
    /// on both palettes, so it stands off the track or the stage under it.
    pub knob: Color32,
    /// What a scrim over the stage is mixed from; its alpha is the most it ever covers.
    pub scrim: Color32,
    /// What every card, menu and window over the stage casts.
    pub shadow: Color32,
}

/// Graphite surfaces, the window's own.
pub(super) const DARK: Palette = Palette {
    sunken: Color32::from_rgb(0x0c, 0x0e, 0x11),
    base: Color32::from_rgb(0x11, 0x14, 0x18),
    panel: Color32::from_rgb(0x16, 0x1a, 0x1f),
    raised: Color32::from_rgb(0x1e, 0x23, 0x29),
    hover: Color32::from_rgb(0x27, 0x2d, 0x34),
    active: Color32::from_rgb(0x31, 0x38, 0x40),
    hairline: Color32::from_rgb(0x25, 0x2a, 0x31),
    line: Color32::from_rgb(0x39, 0x41, 0x4b),
    text_high: Color32::from_rgb(0xef, 0xeb, 0xe5),
    text_mid: Color32::from_rgb(0xa9, 0xa7, 0xa3),
    text_low: Color32::from_rgb(0x8a, 0x8e, 0x94),
    accent: Color32::from_rgb(0xf2, 0x76, 0x3a),
    accent_deep: Color32::from_rgb(0xc9, 0x53, 0x1f),
    accent_soft: Color32::from_rgb(0xff, 0xb1, 0x84),
    // 14 % of the accent: the wash behind an active segment or tool.
    accent_wash: Color32::from_rgba_unmultiplied_const(0xf2, 0x76, 0x3a, 36),
    on_accent: Color32::from_rgb(0x1a, 0x0e, 0x07),
    ok: Color32::from_rgb(0x4f, 0xcf, 0x8a),
    warn: Color32::from_rgb(0xe8, 0xb0, 0x43),
    danger: Color32::from_rgb(0xef, 0x5b, 0x4b),
    danger_wash: Color32::from_rgba_unmultiplied_const(0xef, 0x5b, 0x4b, 31),
    warn_wash: Color32::from_rgba_unmultiplied_const(0xe8, 0xb0, 0x43, 36),
    ok_wash: Color32::from_rgba_unmultiplied_const(0x4f, 0xcf, 0x8a, 36),
    axis: [
        Color32::from_rgb(0xe0, 0x57, 0x4f),
        Color32::from_rgb(0x5f, 0xbf, 0x62),
        Color32::from_rgb(0x4f, 0x9c, 0xff),
    ],
    picked: Color32::from_rgb(0xff, 0xb1, 0x84),
    picked_wash: Color32::from_rgba_unmultiplied_const(0xf2, 0x76, 0x3a, 26),
    knob: Color32::from_rgb(0xef, 0xeb, 0xe5),
    scrim: Color32::from_rgb(0x05, 0x06, 0x08),
    shadow: Color32::from_black_alpha(115),
};

/// Slate: cool blue-grey surfaces under a white panel, so the ember accent stands apart.
/// Raised surfaces darken rather than lighten; see ADR 0383.
pub(super) const LIGHT: Palette = Palette {
    sunken: Color32::from_rgb(0xe2, 0xe6, 0xeb),
    base: Color32::from_rgb(0xef, 0xf2, 0xf5),
    panel: Color32::from_rgb(0xff, 0xff, 0xff),
    raised: Color32::from_rgb(0xf1, 0xf3, 0xf6),
    hover: Color32::from_rgb(0xe8, 0xec, 0xf0),
    active: Color32::from_rgb(0xdb, 0xe0, 0xe6),
    hairline: Color32::from_rgb(0xdf, 0xe3, 0xe8),
    line: Color32::from_rgb(0xc0, 0xc7, 0xd0),
    text_high: Color32::from_rgb(0x15, 0x1a, 0x21),
    text_mid: Color32::from_rgb(0x44, 0x4c, 0x57),
    text_low: Color32::from_rgb(0x5c, 0x65, 0x71),
    accent: Color32::from_rgb(0xec, 0x6b, 0x2d),
    accent_deep: Color32::from_rgb(0xc4, 0x50, 0x1b),
    // On white the accent is too pale for text: what reads as accent is a burnt orange.
    accent_soft: Color32::from_rgb(0xb0, 0x45, 0x12),
    accent_wash: Color32::from_rgba_unmultiplied_const(0xec, 0x6b, 0x2d, 33),
    on_accent: Color32::from_rgb(0x1a, 0x0e, 0x07),
    ok: Color32::from_rgb(0x18, 0x70, 0x3f),
    warn: Color32::from_rgb(0x8f, 0x5d, 0x00),
    danger: Color32::from_rgb(0xc3, 0x34, 0x2a),
    danger_wash: Color32::from_rgba_unmultiplied_const(0xc3, 0x34, 0x2a, 26),
    warn_wash: Color32::from_rgba_unmultiplied_const(0xc4, 0x8a, 0x14, 36),
    ok_wash: Color32::from_rgba_unmultiplied_const(0x1d, 0x7f, 0x49, 26),
    axis: [
        Color32::from_rgb(0xcf, 0x3f, 0x37),
        Color32::from_rgb(0x2f, 0x8f, 0x35),
        Color32::from_rgb(0x2a, 0x74, 0xd9),
    ],
    picked: Color32::from_rgb(0xb0, 0x45, 0x12),
    picked_wash: Color32::from_rgba_unmultiplied_const(0xec, 0x6b, 0x2d, 23),
    knob: Color32::from_rgb(0xff, 0xff, 0xff),
    scrim: Color32::from_rgba_unmultiplied_const(0x14, 0x1e, 0x2d, 115),
    shadow: Color32::from_rgba_unmultiplied_const(0x14, 0x1e, 0x2d, 26),
};
