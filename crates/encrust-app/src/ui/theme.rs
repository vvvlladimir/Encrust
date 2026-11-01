// The one file a colour may be spelled in; `clippy.toml` keeps the rest of the crate out.
#![expect(clippy::disallowed_methods, reason = "the palette is defined here")]

use std::cell::Cell;
use std::ops::RangeInclusive;

use egui::epaint::Shadow;
use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle, Theme};

use crate::ui::fonts;

pub mod motion;
mod palette;
mod scene;

pub use palette::Palette;
pub use scene::Scene;

/// Named font families the bundled faces are registered under.
pub const MEDIUM: &str = "plex-sans-medium";
pub const SEMIBOLD: &str = "plex-sans-semibold";

/// Text styles this application adds to egui's own, addressed by name.
pub const SECTION: &str = "section";

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

/// What each group of supports past the first is drawn in, on the plate and beside its
/// row; the first is the scene's own support colour. Cool and apart from the marks.
const SUPPORT_TINTS: [Color32; 5] = [
    Color32::from_rgb(0x3f, 0x9e, 0x8c),
    Color32::from_rgb(0x8c, 0x6c, 0xc4),
    Color32::from_rgb(0x6f, 0x9a, 0x45),
    Color32::from_rgb(0xb8, 0x62, 0x9a),
    Color32::from_rgb(0x3f, 0x86, 0xc4),
];

/// The colour supports of `group` are drawn in, cycling once the palette runs out.
pub fn support_tint(group: usize) -> Color32 {
    match group % (SUPPORT_TINTS.len() + 1) {
        0 => scene().support,
        tint => SUPPORT_TINTS[tint - 1],
    }
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

thread_local! {
    /// Whether this frame paints light: set by `follow`, read by `colors` and `scene`. Per
    /// thread, so two windows drawn on two threads, as tests do, keep their own.
    static LIGHT: Cell<bool> = const { Cell::new(false) };
}

/// Hands egui the user's `preference` and points the window at the palette it resolves,
/// the system's when the user left it to the system. Called once a frame, before drawing.
pub fn follow(ctx: &egui::Context, preference: egui::ThemePreference) {
    if ctx.options(|options| options.theme_preference) != preference {
        ctx.set_theme(preference);
    }
    LIGHT.set(ctx.theme() == Theme::Light);
}

/// The set of colours the viewport paints with this frame.
pub fn scene() -> &'static Scene {
    if LIGHT.get() {
        &scene::LIGHT
    } else {
        &scene::DARK
    }
}

/// How much of itself a surface keeps at most when the viewport is drawing the models
/// seen through — on an edge turned away from the camera. A surface facing the camera
/// keeps a third of this, so a wall is glass rather than a window; the shader's
/// `XRAY_FLOOR` is that third.
pub const SEEN_THROUGH: f32 = 0.80;

/// Points across an edge a tool marks on a model: wide enough to aim a click at.
pub const MARK_LINE_W: f32 = 3.0;

/// A token as the vertex and uniform buffers want it: gamma-space, not linear.
///
/// `egui-wgpu` writes gamma to a non-sRGB target and our pipeline shares that target, so
/// a decode here would wash the viewport out. See `docs/design/ui-design-system.md`.
pub fn gamma(color: Color32) -> [f32; 4] {
    color.to_normalized_gamma_f32()
}

/// The palette egui's `theme` paints with, whichever is in force: for a preview of it.
pub fn palette_of(theme: Theme) -> &'static Palette {
    match theme {
        Theme::Light => &palette::LIGHT,
        Theme::Dark => &palette::DARK,
    }
}

/// The palette the window paints with this frame.
pub fn colors() -> &'static Palette {
    if LIGHT.get() {
        &palette::LIGHT
    } else {
        &palette::DARK
    }
}

/// Heights, widths and gaps, in points. One density, applied everywhere.
pub const ROW_H: f32 = 30.0;
/// A model's row in the models card, and the printer's row over the plate's Slice.
pub const MODEL_ROW_H: f32 = 32.0;
pub const PRINTER_ROW_H: f32 = 34.0;
/// Every box a value is typed in stands as tall as a button.
pub const FIELD_H: f32 = BUTTON_H;
/// Width of the value box in a named row.
pub const FIELD_W: f32 = 128.0;
pub const PANEL_PAD: f32 = 12.0;
pub const ITEM_GAP: f32 = 8.0;
/// Between the rows of a section's body.
pub const ROW_GAP: f32 = 6.0;
/// Between buttons sharing one row, and between a tool heading's mode icons.
pub const BUTTON_GAP: f32 = 6.0;
pub const ICON_GAP: f32 = 2.0;
/// Between a reading and the row of buttons under it.
pub const NOTE_GAP: f32 = 4.0;
/// Between the groups of a sheet or a page.
pub const SECTION_GAP: f32 = 14.0;
/// Between the start page's title and the zone under it.
pub const PAGE_GAP: f32 = 34.0;
/// How far a palette's rows and foot stand in from its sides.
pub const PALETTE_INSET: f32 = 16.0;
/// Height of a section's heading row, which folds it.
pub const SECTION_H: f32 = 36.0;
pub const SCENE_W: f32 = 272.0;
/// The inspector's heading row, which names the open tool.
pub const INSPECTOR_HEAD_H: f32 = 48.0;
/// How far either column may be dragged.
pub const SCENE_W_RANGE: RangeInclusive<f32> = 170.0..=420.0;
pub const INSPECTOR_W_RANGE: RangeInclusive<f32> = 260.0..=440.0;
/// The inspector's width until it is dragged.
pub const INSPECTOR_W: f32 = 316.0;
/// Where Preview divides the model from the mask, as a fraction of the stage's width, and
/// how far the divide may be dragged.
pub const MASK_SPLIT: f32 = 0.5;
pub const MASK_SPLIT_RANGE: RangeInclusive<f32> = 0.15..=0.85;
/// Points between a card floating over the stage and the stage's edge, and between two
/// cards.
pub const FLOAT_GAP: f32 = 8.0;
/// The top bar, and the layer strip down the stage's right edge. On macOS the system
/// draws its window buttons over the bar's left end, so the bar is tall enough for them.
pub const TOP_BAR_H: f32 = 44.0;
/// The least a desktop window may be made: the side cards fit beside a stage still worth
/// working in, and the bar's names are cut rather than lost.
pub const WINDOW_MIN_W: f32 = 900.0;
pub const WINDOW_MIN_H: f32 = 560.0;
pub const LAYER_STRIP_W: f32 = 64.0;
/// A rail button holds a glyph over its name, or stands a square of the glyph alone.
pub const RAIL_BUTTON_H: f32 = 50.0;
pub const RAIL_ICON_H: f32 = 36.0;
/// Between a card of buttons, the rail or the layer strip, and its buttons.
pub const TOOLBAR_PAD: f32 = 4.0;
/// The chips and buttons standing in the top bar and the layer strip.
pub const CHIP_H: f32 = 32.0;
/// A list the chip opens, and how far under the chip it hangs.
pub const CHOOSER_W: f32 = 300.0;
pub const CHOOSER_GAP: f32 = 6.0;
pub const ICON_SIZE: f32 = 28.0;
/// The squares of a card's own rows: its plates and the actions under its models.
pub const SMALL_ICON_SIZE: f32 = 26.0;
pub const PRIMARY_H: f32 = 40.0;
/// The ring a control wears while it holds the keyboard's focus, and the halo of the
/// accent's wash around it, points.
pub const FOCUS_W: f32 = 1.0;
pub const FOCUS_HALO_W: f32 = 3.0;
pub const BUTTON_H: f32 = 32.0;
/// Height of a running job's progress bar, and of a print's in the printer sheet.
pub const BAR_H: f32 = 4.0;
pub const SHEET_BAR_H: f32 = 6.0;
/// A form block: its title's column, the most its fields take, and the room between them.
pub const BLOCK_TITLE_W: f32 = 180.0;
pub const BLOCK_FIELDS_W: f32 = 460.0;
pub const BLOCK_GAP: f32 = 24.0;
/// A window over the plate: its title row, the screen it leaves either side, and the list
/// down its left, which gives way to the page beside it in a small window.
pub const DIALOG_HEAD_H: f32 = 48.0;
pub const DIALOG_MARGIN: f32 = 24.0;
pub const DIALOG_LIST_W: f32 = 260.0;
pub const DIALOG_LIST_SHARE: f32 = 0.28;
/// How much a window over the plate dims the stage behind it, and how much Find does.
pub const SCRIM_SHARE: f32 = 0.62;
pub const FIND_SCRIM_SHARE: f32 = 0.4;
/// The bar along the foot of a window's page, and the name column of a form's text row.
pub const DIALOG_BAR_H: f32 = 56.0;
pub const ROW_NAME_W: f32 = 150.0;
/// A window page's title, and the muted second name beside it, pt.
pub const PAGE_TITLE_PT: f32 = 17.0;
pub const PAGE_SUBTITLE_PT: f32 = 15.0;
/// A row of a table, and of a list whose entries carry a second line.
pub const TABLE_ROW_H: f32 = 36.0;
pub const TWO_LINE_ROW_H: f32 = 48.0;

/// Inner margin of an inspector block, a floating card and a card's header row.
pub const PANEL_MARGIN: Margin = Margin::same(12);
pub const CARD_MARGIN: Margin = Margin::symmetric(10, 8);
/// Side padding of the top bar and the layer strip.
pub const STRIP_MARGIN: Margin = Margin::symmetric(8, 0);

/// Controls are rounded by 6 points, cards by 8 and windows by 10; a group of controls
/// nests around theirs.
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
    FontId::proportional(12.5)
}

/// The label of a segment in a row of choices.
pub fn segment() -> FontId {
    FontId::proportional(11.5)
}

/// A unit inside a number's box.
pub fn unit() -> FontId {
    FontId::proportional(11.5)
}

/// A section heading in the inspector or on a floating card.
pub fn section() -> FontId {
    FontId::new(12.5, FontFamily::Name(SEMIBOLD.into()))
}

/// The title of a form block, in the column beside its fields.
pub fn block_title() -> FontId {
    FontId::new(13.0, FontFamily::Name(MEDIUM.into()))
}

/// The stage dimmed under a window over the plate, `share` of the way to black.
pub fn scrim(share: f32) -> Color32 {
    let [r, g, b, most] = colors().scrim.to_srgba_unmultiplied();
    let alpha = share.clamp(0.0, 1.0) * f32::from(most);
    Color32::from_rgba_unmultiplied(r, g, b, alpha as u8)
}

/// The name of a window over the plate.
pub fn dialog_title() -> FontId {
    FontId::new(15.0, FontFamily::Name(SEMIBOLD.into()))
}

/// What a page or a window is about, set over everything on it.
pub fn page_title(size_pt: f32) -> FontId {
    FontId::new(size_pt, FontFamily::Name(SEMIBOLD.into()))
}

/// A path or a template as it is typed, in the code face.
pub fn typed_path() -> FontId {
    code(12.0)
}

/// The line under a page's title that says what the page is for.
pub fn lede() -> FontId {
    FontId::proportional(14.0)
}

/// A figure of the plate card's grid.
pub fn figure() -> FontId {
    FontId::new(15.0, FontFamily::Name(MEDIUM.into()))
}

/// The heads over the columns of a split: Model, Supports.
pub fn split_head() -> FontId {
    FontId::new(12.0, FontFamily::Name(MEDIUM.into()))
}

/// The layer the layer strip stands at, in its box.
pub fn strip_layer() -> FontId {
    FontId::new(16.0, FontFamily::Name(SEMIBOLD.into()))
}

/// The layer strip's height in mm under the layer.
pub fn strip_height() -> FontId {
    FontId::new(10.5, FontFamily::Name(MEDIUM.into()))
}

/// What a tool does with the pointer, and the key that does it, in the hint bar.
pub fn hint() -> FontId {
    FontId::proportional(12.0)
}

pub fn hint_key() -> FontId {
    FontId::new(12.0, FontFamily::Name(MEDIUM.into()))
}

/// The words of the primary button.
pub fn primary() -> FontId {
    FontId::new(13.0, FontFamily::Name(MEDIUM.into()))
}

/// The open tool's name, at the head of the inspector.
pub fn tool_title() -> FontId {
    FontId::new(14.5, FontFamily::Name(SEMIBOLD.into()))
}

/// X, Y or Z beside the arrows in the corner of the plate.
pub fn axis_letter() -> FontId {
    FontId::new(11.0, FontFamily::Name(SEMIBOLD.into()))
}

/// A tool's name under its glyph on the rail.
pub fn rail_name() -> FontId {
    FontId::proportional(10.5)
}

/// A key's face on a keycap.
pub fn keycap() -> FontId {
    FontId::new(10.5, FontFamily::Name(MEDIUM.into()))
}

/// The digit that picks a tool, in the corner of its rail button.
pub fn rail_key() -> FontId {
    FontId::proportional(9.0)
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

/// Installs the fonts and the tokens above on a context, a style for each theme egui may
/// resolve to. Called once, at startup.
pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(fonts::definitions());
    ctx.style_mut_of(Theme::Dark, |into| style(into, Theme::Dark, &palette::DARK));
    ctx.style_mut_of(Theme::Light, |into| {
        style(into, Theme::Light, &palette::LIGHT)
    });
}

fn style(style: &mut egui::Style, theme: Theme, colors: &Palette) {
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

    style.visuals = visuals(theme, colors);
    style.interaction.tooltip_delay = 0.35;
    style.animation_time = motion::FAST.unreduced_s();
    style.interaction.selectable_labels = false;
}

fn visuals(theme: Theme, colors: &Palette) -> egui::Visuals {
    let mut visuals = theme.default_visuals();

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
    visuals.window_shadow = window_shadow_of(colors);
    visuals.popup_shadow = window_shadow_of(colors);

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

/// The one shadow every card over the stage stands on.
pub fn card_shadow() -> Shadow {
    Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: colors().shadow,
    }
}

/// What a window over the plate, a menu and a tooltip cast.
pub fn window_shadow() -> Shadow {
    window_shadow_of(colors())
}

fn window_shadow_of(colors: &Palette) -> Shadow {
    Shadow {
        offset: [0, 10],
        blur: 30,
        spread: 0,
        color: colors.shadow,
    }
}

#[cfg(test)]
mod tests;
