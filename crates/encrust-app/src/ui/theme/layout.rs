//! The sizes of each screen's own parts, points, kept here beside the shared ones so a
//! panel names a size rather than writing it; where each stands: `docs/design/ui-screens.md`.

/// `panels/find.rs`.
pub mod find {
    /// The most rows shown at once: past it the list scrolls.
    pub const LIST_H: f32 = crate::ui::theme::PALETTE_ROW_H * 10.0;
}

/// `panels/inspector/position.rs`.
pub mod inspector_position {
    /// The column the axis letters stand in, in every block, so the fields of all three line up.
    pub const AXIS_W: f32 = 14.0;
    /// Height of the Size and Ratio titles over their columns.
    pub const CAPTION_H: f32 = 16.0;
    /// Width of a ±45° button.
    pub const TURN_W: f32 = 48.0;
}

/// `panels/inspector/relief/picture.rs`.
pub mod inspector_relief_picture {
    /// The edge of the thumbnail a chosen picture is shown at, points.
    pub const THUMB_PT: f32 = 56.0;
    pub const GALLERY_GAP: f32 = 6.0;
}

/// `panels/inspector/support_picker.rs`.
pub mod inspector_support_picker {
    /// How far the shapes of the automatic preset stand in under it, points.
    pub const SHAPE_INDENT: f32 = 20.0;
}

/// `panels/inspector/support_picker/list.rs`.
pub mod inspector_support_picker_list {
    /// The room above the foot's rule.
    pub const FOOT_GAP: f32 = 4.0;
}

/// `panels/inspector/text.rs`.
pub mod inspector_text {
    /// How tall the list of families stands, points: some ten names.
    pub const LIST_H: f32 = 280.0;
}

/// `panels/mask_pane.rs`.
pub mod mask_pane {
    /// Points between the mask and the edge of the pane it is centred in.
    pub const INSET: f32 = 18.0;
    /// The ring around an issue on the shown layer, points: wide enough to find a pixel by.
    pub const RISK_RING_R: f32 = 12.0;
}

/// `panels/mask_pane/over.rs`.
pub mod mask_pane_over {
    /// Points between the zoom and the tag under it.
    pub const TAG_GAP: f32 = 6.0;
}

/// `panels/plate_summary.rs`.
pub mod plate_summary {
    /// Room around the card's heading and figures, over its foot.
    pub const BODY_MARGIN: egui::Margin = egui::Margin {
        left: 12,
        right: 12,
        top: 0,
        bottom: 8,
    };
    /// Room between the figures and the Details row, and under the word on a broken model,
    /// points.
    pub const FIGURES_GAP: f32 = 8.0;
}

/// `panels/printer_card/history.rs`.
pub mod printer_card_history {
    pub const TILE_GAP: f32 = 10.0;
    pub const TILE_H: f32 = 60.0;
    pub const PICTURE: f32 = 44.0;
}

/// `panels/printer_card/ring.rs`.
pub mod printer_card_ring {
    /// The ring's box, as tall as the chip beside it, and how thick its track is drawn.
    pub const SIZE: f32 = crate::ui::theme::CHIP_H;
    pub const TRACK_W: f32 = 3.0;
    /// The ring in a row of the list.
    pub const SMALL: f32 = 22.0;
    /// Points between the ring and the chip.
    pub const GAP: f32 = 6.0;
    /// How wide a row of the list is drawn.
    pub const ROW_W: f32 = 300.0;
    pub const ROW_H: f32 = 46.0;
}

/// `panels/printer_card/sheet.rs`.
pub mod printer_card_sheet {
    /// How wide the sheet is, and the picture of the plate at its left.
    pub const SHEET_W: f32 = 620.0;
    pub const PICTURE: egui::Vec2 = egui::vec2(240.0, 180.0);
    pub const COLUMN_GAP: f32 = 18.0;
    /// What the sheet leaves of the window's height for its margin, and the least of the
    /// history it shows before that scrolls.
    pub const CHROME_H: f32 = 120.0;
    pub const HISTORY_MIN_H: f32 = 96.0;
}

/// `panels/report.rs`.
pub mod report {
    /// Inside the box the report stands in.
    pub const REPORT_MARGIN: egui::Margin = egui::Margin::same(8);
    /// The sheet's width where there is room for it, and the narrowest it is still drawn at.
    pub const SHEET_W: f32 = 520.0;
    pub const SHEET_MIN_W: f32 = 300.0;
    /// Left around the sheet so it never reaches the window's own edges.
    pub const SCREEN_MARGIN: f32 = 96.0;
    /// How tall a typed answer is drawn before it scrolls.
    pub const ANSWER_H: f32 = 56.0;
    /// The scrolling body between the header and the buttons: its bounds, and the height those
    /// two take out of the window before it is measured.
    pub const BODY_MAX_H: f32 = 420.0;
    pub const BODY_MIN_H: f32 = 160.0;
    pub const CHROME_H: f32 = 110.0;
    pub const REPORT_MIN_H: f32 = 120.0;
}

/// `panels/scene_panel.rs`.
pub mod scene_panel {
    /// The heading over the list, as tall as a section's.
    pub const HEADER_H: f32 = crate::ui::theme::SECTION_H;
    /// How wide the import report is allowed to get before it wraps.
    pub const POPUP_W: f32 = 230.0;
    /// Points around the row of plates at the card's head, which stands 42 tall.
    pub const PLATES_MARGIN: egui::Margin = egui::Margin::symmetric(12, 8);
}

/// `panels/section.rs`.
pub mod section {
    /// The room between the strip's groups.
    pub const GAP: f32 = 10.0;
}

/// `panels/settings.rs`.
pub mod settings {
    /// Room over a group name of the list but the first, and under the find line.
    pub const GROUP_GAP: f32 = 10.0;
    pub const FIND_GAP: f32 = 12.0;
    /// Room above and below the fields of a block.
    pub const BLOCK_PAD: f32 = 18.0;
}

/// `panels/settings/appearance.rs`.
pub mod settings_appearance {
    /// Between two theme cards, points.
    pub const CARD_GAP: f32 = 10.0;
}

/// `panels/settings/compensation.rs`.
pub mod settings_compensation {
    /// Width of a box in the calculators, points.
    pub const SIZE_W: f32 = 112.0;
    pub const CLOCK_W: f32 = 84.0;
}

/// `panels/settings/general.rs`.
pub mod settings_general {
    /// What a path box leaves for the buttons beside it, points.
    pub const PATH_BUTTONS_W: f32 = 170.0;
    /// What the template's box leaves for Reset beside it, points.
    pub const RESET_W: f32 = 40.0;
}

/// `panels/settings/jump.rs`.
pub mod settings_jump {
    /// Points under the top of the view within which a section counts as the one in view.
    pub const IN_VIEW: f32 = 24.0;
}

/// `panels/settings/machines.rs`.
pub mod settings_machines {
    /// Between the tabs over a machine.
    pub const TAB_GAP: f32 = 18.0;
    /// Room between two zones offered in an empty list.
    pub const ZONE_GAP: f32 = 12.0;
}

/// `panels/settings/machines/resins.rs`.
pub mod settings_machines_resins {
    /// The row of filters over the table.
    pub const FILTERS_H: f32 = 52.0;
    /// The Add a resin menu: wide enough for a resin's name beside its delete button, and no
    /// wider, because a row with its button at the right end takes whatever it is offered.
    pub const MENU_W: f32 = 240.0;
}

/// `panels/settings/printer/connection.rs`.
pub mod settings_printer_connection {
    /// Inside a listed printer's row.
    pub const LISTED_MARGIN: egui::Margin = egui::Margin::symmetric(8, 4);
    /// How wide a field here is: enough for a host name, not enough to stretch the card.
    pub const FIELD_W: f32 = 230.0;
    /// How wide a listed printer's name is edited.
    pub const NAME_W: f32 = 170.0;
}

/// `panels/settings/resin.rs`.
pub mod settings_resin {
    /// A dropdown holding a currency or `kg`, points: room for the symbol and its caret.
    pub const SMALL_DROPDOWN_W: f32 = 64.0;
}

/// `panels/settings/supports.rs`.
pub mod settings_supports {
    /// The band the tabs over the form stand in, and the tabs themselves.
    pub const TABS_H: f32 = 52.0;
    pub const TAB_ROW_H: f32 = 28.0;
}

/// `panels/stage.rs`.
pub mod stage {
    /// Points of grab either side of the divide, and the least of the model or the mask left
    /// between it and the side cards.
    pub const GRAB_W: f32 = 4.0;
    pub const SIDE_MIN: f32 = 120.0;
}

/// `panels/stage_notice.rs`.
pub mod stage_notice {
    /// Points between the cards, and how far above the stage's foot the lowest stands.
    pub const GAP: f32 = 6.0;
    pub const FOOT_RISE: f32 = 8.0;
    /// The widest a message may grow before it wraps.
    pub const MESSAGE_W: f32 = 440.0;
    /// Points a notice slides up as it comes in.
    pub const COME_IN: f32 = 6.0;
}

/// `panels/start.rs`.
pub mod start {
    /// Between the notes over the drop zone and the zone.
    pub const ZONE_TOP: f32 = 28.0;
    /// The page's room under the top bar and either side of it.
    pub const PAD_TOP: f32 = 48.0;
    pub const PAD_SIDE: f32 = 140.0;
    /// Tiles stand as many to a row as fit at their least width, the gap between two.
    pub const TILE_MIN_W: f32 = 180.0;
    pub const TILE_GAP: f32 = 14.0;
    /// The drop zone: its height, its edge's dashes and weight, its corner, and the square of
    /// its glyph.
    pub const ZONE_H: f32 = 104.0;
    /// The switch between the two shelves.
    pub const SHELF_SWITCH_W: f32 = 220.0;
    /// A tile's picture, and the room under it for the name and the two words.
    pub const CAPTION_H: f32 = 52.0;
}

/// `panels/title_bar.rs`.
pub mod title_bar {
    /// The room either side of what a bar control says, and the room kept between the two
    /// ends of the bar.
    pub const PAD: f32 = 10.0;
    pub const ENDS_GAP: f32 = 12.0;
    /// A bar narrower than this shows the right end's controls as their glyphs alone.
    pub const COMPACT_BAR_W: f32 = 720.0;
}

/// `panels/title_bar/chip.rs`.
pub mod title_bar_chip {
    /// A bar narrower than this names the machine without its maker.
    pub const SHORT_NAMES_W: f32 = 1100.0;
    /// The least of a name the chip shows: enough for a letter and the ellipsis.
    pub const NAME_LEAST_W: f32 = 24.0;
}

/// `panels/title_bar/menus.rs`.
pub mod title_bar_menus {
    /// Points inside a menu title either side of its name, and between two titles.
    pub const MENU_PAD: f32 = 12.0;
    pub const MENU_GAP: f32 = 4.0;
    /// How wide the list of recent files stands at least, points: room for a name and its age.
    pub const RECENT_W: f32 = 250.0;
}

/// `panels/tool_rail.rs`.
pub mod tool_rail {
    /// The least a named button is wide: narrower, the names go (`ui-screens.md`, Tool row).
    pub const NAMED_MIN_W: f32 = 56.0;
    /// The most a named button is wide: its height, or the least a name needs if that is more.
    pub const NAMED_MAX_W: f32 = crate::ui::theme::RAIL_BUTTON_H.max(NAMED_MIN_W);
}

/// `panels/update_sheet.rs`.
pub mod update_sheet {
    /// The sheet's width, the room left round it, and the height of what is not the changes.
    pub const SHEET_W: f32 = 640.0;
    pub const SCREEN_MARGIN: f32 = 96.0;
    pub const CHROME_H: f32 = 230.0;
    pub const CHANGES_MAX_H: f32 = 380.0;
    pub const CHANGES_MIN_H: f32 = 120.0;
    /// Room kept at the hero's right for its button.
    pub const HERO_BUTTON_W: f32 = 140.0;
    /// Room above a group of the changes but the first.
    pub const GROUP_GAP: f32 = 12.0;
}
