//! A test print kept in the repository, as a tile: fetched on the first click with its bar
//! along the picture while it comes, put on the plate on the next; see `docs/decisions/0394`.

use egui::Ui;

use crate::files::Handed;
use crate::i18n::tr;
use crate::panels::Window;
use crate::samples::remote::{Fetches, Fetching, REMOTES};
use crate::ui::{PictureTile, picture_tile};

/// The `index`th remote test print as a tile: to fetch, coming, or fetched.
pub fn tile(ui: &mut Ui, fetches: &Fetches, index: usize, size: egui::Vec2) -> egui::Response {
    let remote = &REMOTES[index];
    let name = remote.name();
    let shown = PictureTile {
        name: &name,
        left: tr!("start-test-print"),
        right: state(fetches, index),
        picture: picture(ui.ctx(), index),
        fetching: fetches.get(index).map(Fetching::share),
    };
    picture_tile(ui, &shown, size).on_hover_text(remote.about())
}

/// Puts the `index`th remote test print on the plate once fetched, or starts fetching it; a
/// browser, which keeps no files, opens it in a tab of its own instead.
pub fn picked(ui: &Ui, window: &mut Window, index: usize) {
    let remote = &REMOTES[index];
    if let Some(path) = remote.fetched() {
        crate::app::open_by_what_it_is(window, Handed::Path(path));
        return;
    }
    match remote.path() {
        Some(_) => window.machine.fetches.start(index),
        None => ui.ctx().open_url(egui::OpenUrl::new_tab(remote.url())),
    }
}

/// Where the `index`th remote test print stands, for its tile's kind and its menu row:
/// to fetch, its share or Downloading while it comes, or STL once fetched.
pub fn state(fetches: &Fetches, index: usize) -> String {
    match (
        fetches.get(index).map(Fetching::share),
        REMOTES[index].fetched(),
    ) {
        (Some(Some(share)), _) => tr!("value-percent", value = (share * 100.0).round() as u32),
        (Some(None), _) => tr!("start-downloading"),
        (None, Some(_)) => "STL".to_owned(),
        (None, None) => tr!("start-download"),
    }
}

/// The picture shipped with the `index`th remote test print, once a window and theme.
fn picture(ctx: &egui::Context, index: usize) -> Option<egui::TextureHandle> {
    let id = egui::Id::new(("remote-picture", index, ctx.theme()));
    if let Some(known) = ctx.data(|data| data.get_temp::<Option<egui::TextureHandle>>(id)) {
        return known;
    }
    let remote = &REMOTES[index];
    let texture = crate::recent::picture::decoded(remote.picture_png)
        .map(|image| ctx.load_texture(remote.name(), image, egui::TextureOptions::LINEAR));
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    texture
}
