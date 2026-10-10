// The one file a colour may be spelled in; `clippy.toml` keeps the rest of the crate out.
#![expect(clippy::disallowed_methods, reason = "the palette is defined here")]

use std::ops::RangeInclusive;

use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle};

use crate::ui::fonts;

/// Named font families the bundled faces are registered under.
pub const MEDIUM: &str = "plex-sans-medium";
pub const SEMIBOLD: &str = "plex-sans-semibold";

/// Text styles this application adds to egui's own, addressed by name.
pub const SECTION: &str = "section";

/// Every colour the window is allowed to paint with.
///
/// Graphite surfaces, one ember accent, and colours kept for state. See
/// `docs/design/ui-design-system.md`.
pub struct Palette {
    /// Viewport backdrop and any trough a control is sunk into.
    pub sunken: Color32,
    /// The logo's own background: window chrome, title and status strips.
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
    /// The washes behind a verdict a print fails on, or only might, 14 % of each.
    pub danger_wash: Color32,
    pub warn_wash: Color32,
    /// Transform field letters, in the CAD convention X red, Y green, Z blue.
    pub axis: [Color32; 3],
    /// A picked model: the part of a support the pointer has hold of, the row of a
    /// selected model, and the tally that counts them.
    pub picked: Color32,
    /// The wash behind a selected row, 10 % of the accent.
    pub picked_wash: Color32,
}

const ENCRUST: Palette = Palette {
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
    danger_wash: Color32::from_rgba_unmultiplied_const(0xef, 0x5b, 0x4b, 36),
    warn_wash: Color32::from_rgba_unmultiplied_const(0xe8, 0xb0, 0x43, 36),
    axis: [
        Color32::from_rgb(0xe0, 0x57, 0x4f),
        Color32::from_rgb(0x5f, 0xbf, 0x62),
        Color32::from_rgb(0x4f, 0x9c, 0xff),
    ],
    picked: Color32::from_rgb(0xff, 0xb1, 0x84),
    picked_wash: Color32::from_rgba_unmultiplied_const(0xf2, 0x76, 0x3a, 26),
};

/// What an exposure band is tinted with, on the model and beside its own fields. Six
/// hues that stay apart from the model's grey and from the accent.
const BAND_TINTS: [Color32; 6] = [
    Color32::from_rgb(0x4f, 0x8e, 0xc9),
    Color32::from_rgb(0x7a, 0xa8, 0x5c),
    Color32::from_rgb(0xc9, 0x7f, 0xd1),
    Color32::from_rgb(0xe0, 0xa3, 0x3a),
    Color32::from_rgb(0x4f, 0xc9, 0xbb),
    Color32::from_rgb(0xd1, 0x58, 0x4c),
];

/// The tint of band `index`, cycling once the palette runs out.
pub fn band_tint(index: usize) -> Color32 {
    BAND_TINTS[index % BAND_TINTS.len()]
}

/// What each group of supports is drawn in, on the plate and beside its row. The first is
/// the scene's own support colour; the rest stay cool and apart from the marks.
const SUPPORT_TINTS: [Color32; 6] = [
    ENCRUST_SCENE.support,
    Color32::from_rgb(0x3f, 0x9e, 0x8c),
    Color32::from_rgb(0x8c, 0x6c, 0xc4),
    Color32::from_rgb(0x6f, 0x9a, 0x45),
    Color32::from_rgb(0xb8, 0x62, 0x9a),
    Color32::from_rgb(0x3f, 0x86, 0xc4),
];

/// The colour supports of `group` are drawn in, cycling once the palette runs out.
pub fn support_tint(group: usize) -> Color32 {
    SUPPORT_TINTS[group % SUPPORT_TINTS.len()]
}

/// The colour a resin's own profile gives it, for its swatch: data the user chose, not a
/// colour of the window.
pub fn resin_swatch([r, g, b]: [u8; 3]) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// A pixel of a layer mask: its exposure as grey, or the danger colour where it cures over
/// nothing.
pub fn mask_pixel(grey: u8, island: bool) -> Color32 {
    if island && grey > 0 {
        colors().danger
    } else {
        Color32::from_gray(grey)
    }
}

/// Every colour the viewport paints the plate and what stands on it with.
///
/// Separate from the `Palette` because none of it is window chrome: it is the grey of
/// printed resin and the marks laid over it, and the luminance ladder the surfaces are
/// tested against does not apply. Reaches the shader through `gamma`.
pub struct Scene {
    /// Unmarked model surface: a cool resin, so the warm marks and the selection read on it.
    pub object: Color32,
    /// A warm light grey apart from the model's blue, so scaffolding does not read as part
    /// of the part.
    pub support: Color32,
    /// A model or a support in the selection.
    pub selected: Color32,
    /// A mesh that could not be oriented outward.
    pub unsound: Color32,
    /// A hollowing blocker: a marker the user placed, not geometry that is printed.
    /// Translucent and apart from every colour a model is drawn in, so it never reads as
    /// a lump of the part.
    pub blocker: Color32,
    /// The space inside a model that fills with resin no hole lets out. Laid down twice —
    /// the near wall of the cavity and its far wall both paint it — so it is translucent
    /// enough that the lattice standing in it still reads. Cold rather than warm, so it
    /// parts from the amber a picked model is drawn in as well as from the grey of one
    /// that is not.
    pub trapped: Color32,
    /// The patch painted to be filled with supports.
    pub painted: Color32,
    /// The patch painted to keep supports off. Violet, not red: an overhang is already
    /// washed red, and what is closed to supports has to read apart from what hangs.
    pub blocked: Color32,
    /// The face a cut leaves behind, flat-lit. See `docs/decisions/0062`.
    pub section_cap: Color32,
    /// What the inside of a model shows as, through a cut or a drain hole.
    pub section_wash: Color32,
    /// A surface leaning far enough to need holding up.
    pub overhang: Color32,
    /// Whatever stands past the build volume, which the printer cannot reach.
    pub outside: Color32,
    /// The plate's own grid, its outline, and the wireframe of the build volume.
    pub grid: Color32,
    pub plate_border: Color32,
    pub volume: Color32,
    /// The build platform the models stand on, and the arm it hangs from. Translucent,
    /// so what stands on the plate is never hidden by it.
    pub platform: Color32,
    /// The word written on the platform's front lip.
    pub label: Color32,
    /// The brackets on the corners of a picked model's bounds.
    pub bounds: Color32,
    /// The flat face Orient to Face would lay on the plate, over the model.
    pub facet: Color32,
    /// Where the Cut tool's plane crosses the model: darker than the selection it is
    /// traced over, and apart from the red that marks a model past the build volume.
    pub cut_line: Color32,
    /// The plate's X and Y lines, in the same CAD convention as `Palette::axis`.
    pub plate_axis: [Color32; 2],
    /// The move and rotate handles, per axis, and the one being hovered or dragged:
    /// brighter than the field letters, since they are drawn over a lit model.
    pub gizmo: [Color32; 3],
    pub gizmo_hot: Color32,
}

const ENCRUST_SCENE: Scene = Scene {
    object: Color32::from_rgb(0x7f, 0xa6, 0xb6),
    support: Color32::from_rgb(0xc9, 0xc2, 0xb6),
    selected: Color32::from_rgb(0xf5, 0xb5, 0x4a),
    unsound: Color32::from_rgb(0xd9, 0x6b, 0x61),
    blocker: Color32::from_rgba_premultiplied(0x6a, 0x54, 0xa8, 0xcc),
    trapped: Color32::from_rgba_premultiplied(0x51, 0x0a, 0x1a, 0x59),
    painted: Color32::from_rgb(0x59, 0xb8, 0x94),
    blocked: Color32::from_rgb(0xa7, 0x64, 0xd6),
    section_cap: Color32::from_rgb(0xff, 0xb1, 0x84),
    section_wash: Color32::from_rgb(0xed, 0xe6, 0xdb),
    overhang: Color32::from_rgb(0xf0, 0x61, 0x40),
    outside: Color32::from_rgb(0xe0, 0x2f, 0x2f),
    grid: Color32::from_rgb(0x4d, 0x52, 0x5c),
    plate_border: Color32::from_rgb(0xb3, 0xb8, 0xc7),
    volume: Color32::from_rgb(0x3d, 0x42, 0x4d),
    platform: Color32::from_rgba_premultiplied(0x34, 0x36, 0x3b, 0x4d),
    label: Color32::from_rgba_premultiplied(0x96, 0x9b, 0xa6, 0xc8),
    bounds: Color32::from_rgb(0xf2, 0xf2, 0xf2),
    facet: Color32::from_rgba_premultiplied(0xd9, 0x6b, 0x33, 0xd9),
    cut_line: Color32::from_rgb(0xa8, 0x3a, 0x12),
    plate_axis: [
        Color32::from_rgb(0xcc, 0x4d, 0x4d),
        Color32::from_rgb(0x59, 0xb3, 0x59),
    ],
    gizmo: [
        Color32::from_rgb(0xff, 0x4f, 0x4f),
        Color32::from_rgb(0x5f, 0xe0, 0x5f),
        Color32::from_rgb(0x4f, 0xa3, 0xff),
    ],
    gizmo_hot: Color32::from_rgb(0xff, 0xe0, 0x66),
};

/// The one set of colours the viewport paints with.
pub const fn scene() -> &'static Scene {
    &ENCRUST_SCENE
}

/// How much of itself a surface keeps at most when the viewport is drawing the models
/// seen through — on an edge turned away from the camera. A surface facing the camera
/// keeps a third of this, so a wall is glass rather than a window; the shader's
/// `XRAY_FLOOR` is that third.
pub const SEEN_THROUGH: f32 = 0.80;

/// A token as the vertex and uniform buffers want it: gamma-space, not linear.
///
/// `egui-wgpu` writes gamma to a non-sRGB target and our pipeline shares that target, so
/// a decode here would wash the viewport out. See `docs/design/ui-design-system.md`.
pub fn gamma(color: Color32) -> [f32; 4] {
    color.to_normalized_gamma_f32()
}

/// The one palette the window paints with.
pub const fn colors() -> &'static Palette {
    &ENCRUST
}

/// Heights, widths and gaps, in points. One density, applied everywhere.
pub const ROW_H: f32 = 30.0;
pub const FIELD_H: f32 = 28.0;
/// Width of the value box in a named row.
pub const FIELD_W: f32 = 128.0;
pub const PANEL_PAD: f32 = 12.0;
pub const ITEM_GAP: f32 = 8.0;
/// Height of a section's heading row, which folds it.
pub const SECTION_H: f32 = 36.0;
pub const RAIL_W: f32 = 68.0;
pub const SCENE_W: f32 = 272.0;
pub const INSPECTOR_W: f32 = 328.0;
/// How far either column may be dragged, and how thin the folded plate's own edge is.
pub const SCENE_W_RANGE: RangeInclusive<f32> = 170.0..=420.0;
pub const INSPECTOR_W_RANGE: RangeInclusive<f32> = 260.0..=440.0;
pub const EDGE_W: f32 = 6.0;
/// On macOS the strip is exactly the native title bar the system still lays the traffic
/// lights out in, so our menus sit on their line. Elsewhere it holds our own buttons.
#[cfg(target_os = "macos")]
pub const TITLE_H: f32 = 28.0;
#[cfg(not(target_os = "macos"))]
pub const TITLE_H: f32 = 36.0;
pub const PLATE_BAR_H: f32 = 30.0;
pub const STATUS_H: f32 = 26.0;
pub const TOOL_SIZE: f32 = 40.0;
pub const ICON_SIZE: f32 = 28.0;
pub const PRIMARY_H: f32 = 40.0;
pub const BUTTON_H: f32 = 32.0;
/// Height of a running job's progress bar.
pub const BAR_H: f32 = 6.0;

/// Inner margin of an inspector block, a floating card and a card's header row.
pub const PANEL_MARGIN: Margin = Margin::same(12);
pub const CARD_MARGIN: Margin = Margin::symmetric(10, 8);
/// Side padding of the title and status strips.
pub const STRIP_MARGIN: Margin = Margin::symmetric(10, 0);

/// Controls are rounded by 6 points, cards by 8 and windows by 10; nothing else is rounded.
pub const R_CONTROL: CornerRadius = CornerRadius::same(6);
pub const R_SURFACE: CornerRadius = CornerRadius::same(8);
pub const R_PANEL: CornerRadius = CornerRadius::same(10);

/// Body text.
pub fn body() -> FontId {
    FontId::proportional(13.0)
}

/// Field labels and the text inside controls.
pub fn label() -> FontId {
    FontId::proportional(12.5)
}

/// Units, hints and counts.
pub fn small() -> FontId {
    FontId::proportional(11.0)
}

/// Both halves of a reading: the name and the figure after its leader.
pub fn reading() -> FontId {
    FontId::proportional(12.0)
}

/// The label of a segment in a row of choices.
pub fn segment() -> FontId {
    FontId::proportional(11.5)
}

/// A unit inside a number's box.
pub fn unit() -> FontId {
    FontId::proportional(10.5)
}

/// A section heading in the inspector or on a floating card.
pub fn section() -> FontId {
    FontId::new(12.0, FontFamily::Name(SEMIBOLD.into()))
}

/// Every millimetre, second, layer index, triangle count and byte size: the text face,
/// whose digits are all one width, so a column of them lines up.
pub fn figures(size_pt: f32) -> FontId {
    FontId::proportional(size_pt)
}

/// What is read or typed as code: an address, a file name, a report to paste.
pub fn code(size_pt: f32) -> FontId {
    FontId::monospace(size_pt)
}

/// A Phosphor glyph resolves through the proportional fallback, so an icon is just text.
pub fn icon(size_pt: f32) -> FontId {
    FontId::proportional(size_pt)
}

/// Installs the fonts and the tokens above on a context. Called once, at startup.
pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(fonts::definitions());
    ctx.all_styles_mut(style);
}

fn style(style: &mut egui::Style) {
    let colors = colors();

    style.text_styles = [
        (TextStyle::Body, body()),
        (TextStyle::Button, label()),
        (TextStyle::Small, small()),
        (TextStyle::Monospace, code(12.0)),
        (
            TextStyle::Heading,
            FontId::new(15.0, FontFamily::Name(MEDIUM.into())),
        ),
        (TextStyle::Name(SECTION.into()), section()),
    ]
    .into();

    let spacing = &mut style.spacing;
    spacing.item_spacing = egui::vec2(ITEM_GAP, ITEM_GAP);
    spacing.button_padding = egui::vec2(9.0, 5.0);
    spacing.interact_size = egui::vec2(40.0, FIELD_H);
    spacing.indent = 18.0;
    spacing.slider_width = 120.0;
    spacing.slider_rail_height = 4.0;
    spacing.menu_margin = Margin::same(6);
    spacing.window_margin = Margin::same(PANEL_PAD as i8);
    spacing.scroll.bar_width = 9.0;
    spacing.scroll.bar_inner_margin = 3.0;

    style.visuals = visuals(colors);
    style.interaction.tooltip_delay = 0.35;
    style.interaction.selectable_labels = false;
}

fn visuals(colors: &Palette) -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();

    visuals.panel_fill = colors.panel;
    visuals.window_fill = colors.panel;
    visuals.window_stroke = Stroke::new(1.0, colors.hairline);
    visuals.window_corner_radius = R_PANEL;
    visuals.menu_corner_radius = R_SURFACE;
    visuals.faint_bg_color = colors.raised;
    visuals.extreme_bg_color = colors.sunken;
    visuals.code_bg_color = colors.raised;
    visuals.override_text_color = Some(colors.text_high);
    visuals.weak_text_color = Some(colors.text_low);
    visuals.warn_fg_color = colors.warn;
    visuals.error_fg_color = colors.danger;
    visuals.hyperlink_color = colors.accent;
    visuals.selection = egui::style::Selection {
        bg_fill: colors.accent_wash,
        stroke: Stroke::new(1.0, colors.accent),
    };
    visuals.slider_trailing_fill = true;
    visuals.handle_shape = egui::style::HandleShape::Circle;
    visuals.window_shadow = shadow();
    visuals.popup_shadow = shadow();

    let widgets = &mut visuals.widgets;
    widgets.noninteractive = widget(colors.panel, colors.hairline, colors.text_mid);
    widgets.inactive = widget(colors.raised, colors.hairline, colors.text_high);
    widgets.hovered = widget(colors.hover, colors.line, colors.text_high);
    widgets.active = widget(colors.active, colors.accent, colors.text_high);
    widgets.open = widget(colors.raised, colors.line, colors.text_high);
    // egui grows a hovered widget by default, which shifts a row of fields about.
    for widget in [
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        widget.expansion = 0.0;
    }

    visuals
}

fn widget(fill: Color32, stroke: Color32, text: Color32) -> egui::style::WidgetVisuals {
    egui::style::WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: fill,
        bg_stroke: Stroke::new(1.0, stroke),
        fg_stroke: Stroke::new(1.0, text),
        corner_radius: R_CONTROL,
        expansion: 0.0,
    }
}

/// The one shadow: a floating card, a menu and a tooltip all cast it.
pub fn shadow() -> egui::epaint::Shadow {
    egui::epaint::Shadow {
        offset: [0, 10],
        blur: 30,
        spread: 0,
        color: Color32::from_black_alpha(115),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance of an opaque colour.
    fn luminance(color: Color32) -> f64 {
        let channel = |value: u8| {
            let value = f64::from(value) / 255.0;
            if value <= 0.039_28 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
    }

    fn contrast(a: Color32, b: Color32) -> f64 {
        let (high, low) = {
            let (a, b) = (luminance(a), luminance(b));
            (a.max(b), a.min(b))
        };
        (high + 0.05) / (low + 0.05)
    }

    #[test]
    fn every_text_colour_meets_aa_on_the_panel() {
        let colors = colors();
        // 4.5:1 is WCAG AA for body text, 3:1 for the large or incidental text the low
        // tone is reserved for.
        assert!(contrast(colors.text_high, colors.panel) >= 4.5);
        assert!(contrast(colors.text_mid, colors.panel) >= 4.5);
        assert!(contrast(colors.text_low, colors.panel) >= 3.0);
    }

    #[test]
    fn the_accent_carries_its_own_label() {
        let colors = colors();
        assert!(contrast(colors.accent, colors.panel) >= 4.5);
        assert!(contrast(colors.on_accent, colors.accent) >= 4.5);
    }

    /// The cardinal mistake this helper exists to prevent: a decode to linear would make
    /// mid grey read as 0.216 and wash the whole viewport out.
    #[test]
    fn gamma_does_not_linearise_a_token() {
        let [red, ..] = gamma(Color32::from_gray(128));
        assert!(
            (red - 128.0 / 255.0).abs() < 1e-6,
            "a token reaches the buffers in the gamma space egui-wgpu writes"
        );
    }

    /// A blocker used to be the colour a picked model is drawn in, which left it
    /// indistinguishable from a lump of the model itself.
    #[test]
    fn a_marker_is_its_own_colour_and_lets_the_model_through() {
        let scene = scene();
        for marker in [scene.blocker, scene.trapped] {
            for surface in [scene.object, scene.selected, scene.support, scene.unsound] {
                assert_ne!(marker, surface, "a marker is not a surface of the part");
            }
            assert!(
                marker.a() < u8::MAX,
                "a marker is translucent, so what it covers is still there"
            );
        }
    }

    #[test]
    fn the_surfaces_are_ordered_from_sunken_to_hover() {
        let colors = colors();
        let ladder = [
            colors.sunken,
            colors.base,
            colors.panel,
            colors.raised,
            colors.hover,
            colors.active,
        ];
        for pair in ladder.windows(2) {
            assert!(
                luminance(pair[0]) < luminance(pair[1]),
                "each surface is lighter than the one it sits on"
            );
        }
    }
}
