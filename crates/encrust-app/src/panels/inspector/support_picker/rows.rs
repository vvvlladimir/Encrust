use crate::panels::Window;
use crate::panels::support_fields::saved_as;
use crate::workspace::Tool;
use printer_profiles::Catalogue;

/// One group as the picker reads it: named for the preset it is built to, never for what
/// it was once called.
pub(super) struct Row {
    pub(super) group: u16,
    pub(super) name: String,
    /// Tip contact diameter, mm: what orders the presets lightest first.
    pub(super) weight: f32,
    pub(super) holds: usize,
    pub(super) hidden: bool,
    pub(super) active: bool,
}

/// What a click on a group asked for, done once the group in hand may change.
pub(super) enum Act {
    Choose(u16),
    Toggle(u16),
    Clear(u16),
    Drop(u16),
}

/// A preset chosen from the list, taken up while the picked models' numbers are held.
pub(super) enum Pick {
    Auto(String),
    Preset(String),
    Shape(String),
}

pub(super) fn rows(window: &Window) -> Vec<Row> {
    let tool = &window.tools.supports;
    let rows = tool.groups.iter().enumerate().map(|(index, entry)| {
        let group = index as u16;
        let active = group == tool.active;
        let profile = if active {
            &tool.profile
        } else {
            &entry.profile
        };
        Row {
            group,
            name: profile.name.clone(),
            weight: profile.tip.contact_diameter_mm,
            holds: tool.group_holds(group, &window.doc.scene),
            hidden: entry.hidden,
            active,
        }
    });
    rows.collect()
}

/// One preset of the catalogue as the list offers it: its id, name and weight.
pub(super) struct Offered {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) weight: f32,
}

/// The presets the list offers: the automatic ones, the shapes they build to, and those
/// chosen by hand, each lightest first.
pub(super) struct Presets {
    pub(super) autos: Vec<Offered>,
    pub(super) shapes: Vec<Offered>,
    pub(super) by_hand: Vec<Offered>,
}

pub(super) fn presets(catalogue: &Catalogue) -> Presets {
    let offered = |auto_shape: bool| {
        let mut list: Vec<Offered> = catalogue
            .supports()
            .filter(|entry| catalogue.is_auto_shape(&entry.id) == auto_shape)
            .map(|entry| Offered {
                id: entry.id.clone(),
                name: entry.profile.name.clone(),
                weight: entry.profile.tip.contact_diameter_mm,
            })
            .collect();
        list.sort_by(|a, b| a.weight.total_cmp(&b.weight).then(a.name.cmp(&b.name)));
        list
    };
    let autos = catalogue.autos().map(|entry| Offered {
        id: entry.id.clone(),
        name: entry.profile.name.clone(),
        weight: 0.0,
    });
    Presets {
        autos: autos.collect(),
        shapes: offered(true),
        by_hand: offered(false),
    }
}

/// Which preset is in hand, what the picker calls it, and where Save writes it.
pub(super) struct InHand {
    /// The preset's name, or the group's while one stands in it.
    pub(super) name: String,
    pub(super) edited: bool,
    pub(super) in_auto: bool,
    /// The support profile in hand, by catalogue id, while it is unedited.
    pub(super) matching: Option<String>,
    /// The automatic preset an edit is saved over, while it is the one shown.
    pub(super) auto_source: Option<String>,
    /// The support profile an edit is saved over.
    pub(super) source: Option<String>,
}

pub(super) fn in_hand(window: &Window, active: &Row) -> InHand {
    let catalogue = &window.machine.slicing.catalogue;
    let tool = &window.tools.supports;
    let current = &tool.profile;
    // On Shape the automatic preset opens into the shapes it builds to, each its own.
    let shaping = *window.tool == Tool::Shape;
    let sizing = tool.auto.as_ref().filter(|_| !shaping);
    let matching = catalogue
        .supports()
        .find(|entry| (tool.auto.is_none() || shaping) && entry.profile == *current)
        .map(|entry| entry.id.clone());
    let (name, edited, auto_source) = match sizing {
        Some(auto) => {
            let shipped = catalogue
                .autos()
                .find(|entry| entry.profile.name == auto.name);
            let edited = shipped.is_some_and(|entry| entry.profile != *auto);
            (
                auto.name.clone(),
                edited,
                shipped.map(|entry| entry.id.clone()),
            )
        }
        None => (current.name.clone(), matching.is_none(), None),
    };
    InHand {
        name: if active.holds > 0 && !shaping {
            active.name.clone()
        } else {
            name
        },
        edited,
        in_auto: tool.auto.is_some(),
        matching,
        auto_source,
        source: saved_as(catalogue, current).map(|entry| entry.id.clone()),
    }
}
