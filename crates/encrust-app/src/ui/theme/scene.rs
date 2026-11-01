//! The viewport's two sets of colours, one per palette; `theme::scene` picks the one in force.
#![expect(clippy::disallowed_methods, reason = "the palette is defined here")]

use egui::Color32;

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
    /// What the Repair tool marks on a model: an open edge, a face without area, drawn
    /// twice or crossing another. Acid yellow-green, apart from every other mark and pick.
    pub fault: Color32,
    /// The same marks where the camera is not: a darker olive, so the one in view leads.
    pub fault_rest: Color32,
    /// An open edge picked by hand to be bridged: magenta, opposite the fault's
    /// yellow-green, so a pick reads apart from the edges it was picked among.
    pub fault_picked: Color32,
    /// The face a cut leaves behind, flat-lit. See `docs/decisions/0062`.
    pub section_cap: Color32,
    /// What the inside of a model shows as, through a cut or a drain hole.
    pub section_wash: Color32,
    /// A surface leaning far enough to need holding up.
    pub overhang: Color32,
    /// Whatever stands past the build volume, which the printer cannot reach.
    pub outside: Color32,
    /// The plate's grid: a line every centimetre, and a stronger one every five.
    pub grid_minor: Color32,
    pub grid_major: Color32,
    /// The plate's outline, and the wireframe of the build volume, both translucent so
    /// they recede behind the models.
    pub plate_border: Color32,
    pub volume: Color32,
    /// The dashed outline of the plate drawn at the height the section cuts at.
    pub section_frame: Color32,
    /// The plate's own surface: a shade off the backdrop, so the plate reads as a floor.
    pub plate: Color32,
    /// How dark the plate goes under a model standing on it, as black at that alpha.
    pub shadow: Color32,
    /// What the models are lit by; see `docs/design/viewport.md`.
    pub light: Lighting,
    /// The word written in front of the plate.
    pub label: Color32,
    /// The brackets on the corners of a picked model's bounds.
    pub bounds: Color32,
    /// The flat face Orient to Face would lay on the plate, over the model.
    pub facet: Color32,
    /// Where the Cut tool's plane crosses the model: darker than the selection it is
    /// traced over, and apart from the red that marks a model past the build volume.
    pub cut_line: Color32,
    /// The X, Y and Z arrows in the plate's corner, in the same CAD convention as
    /// `Palette::axis`.
    pub plate_axis: [Color32; 3],
    /// The move and rotate handles, per axis, and the one being hovered or dragged:
    /// brighter than the field letters, since they are drawn over a lit model.
    pub gizmo: [Color32; 3],
    pub gizmo_hot: Color32,
}

/// A sky and a ground the ambient light is mixed from by how far a surface faces up, a key
/// light from above the front left and a weak warm fill from the back right.
pub struct Lighting {
    pub sky: Color32,
    pub ground: Color32,
    pub key: Color32,
    pub fill: Color32,
    /// Directions the two lights travel towards, in plate coordinates, Z up.
    pub key_towards: [f32; 3],
    pub fill_towards: [f32; 3],
}

/// The viewport under the dark palette.
pub(super) const DARK: Scene = Scene {
    object: Color32::from_rgb(0x7f, 0xa6, 0xb6),
    support: Color32::from_rgb(0xc9, 0xc2, 0xb6),
    selected: Color32::from_rgb(0xf5, 0xb5, 0x4a),
    unsound: Color32::from_rgb(0xd9, 0x6b, 0x61),
    blocker: Color32::from_rgba_premultiplied(0x6a, 0x54, 0xa8, 0xcc),
    trapped: Color32::from_rgba_premultiplied(0x51, 0x0a, 0x1a, 0x59),
    painted: Color32::from_rgb(0x59, 0xb8, 0x94),
    blocked: Color32::from_rgb(0xa7, 0x64, 0xd6),
    fault: Color32::from_rgb(0xc8, 0xf0, 0x3c),
    fault_rest: Color32::from_rgb(0x7d, 0x92, 0x2c),
    fault_picked: Color32::from_rgb(0xff, 0x4f, 0xc8),
    section_cap: Color32::from_rgb(0xff, 0xb1, 0x84),
    section_wash: Color32::from_rgb(0xed, 0xe6, 0xdb),
    overhang: Color32::from_rgb(0xf0, 0x61, 0x40),
    outside: Color32::from_rgb(0xe0, 0x2f, 0x2f),
    grid_minor: Color32::from_rgb(0x26, 0x2c, 0x34),
    grid_major: Color32::from_rgb(0x39, 0x41, 0x4b),
    plate_border: Color32::from_rgba_unmultiplied_const(0x8a, 0x93, 0xa3, 0xb3),
    volume: Color32::from_rgba_unmultiplied_const(0x59, 0x61, 0x6d, 0x59),
    section_frame: Color32::from_rgb(0x8a, 0x8e, 0x94),
    plate: Color32::from_rgb(0x14, 0x18, 0x1d),
    shadow: Color32::from_black_alpha(0x8c),
    light: Lighting {
        sky: Color32::from_rgb(0xdf, 0xe7, 0xf2),
        ground: Color32::from_rgb(0x1a, 0x14, 0x10),
        key: Color32::from_rgb(0xff, 0xff, 0xff),
        fill: Color32::from_rgb(0xff, 0xc9, 0xa8),
        key_towards: [0.4, 0.55, -0.85],
        fill_towards: [-0.75, -0.55, -0.3],
    },
    label: Color32::from_rgba_premultiplied(0x96, 0x9b, 0xa6, 0xc8),
    bounds: Color32::from_rgb(0xf2, 0xf2, 0xf2),
    facet: Color32::from_rgba_premultiplied(0xd9, 0x6b, 0x33, 0xd9),
    cut_line: Color32::from_rgb(0xa8, 0x3a, 0x12),
    plate_axis: super::palette::DARK.axis,
    gizmo: [
        Color32::from_rgb(0xff, 0x4f, 0x4f),
        Color32::from_rgb(0x5f, 0xe0, 0x5f),
        Color32::from_rgb(0x4f, 0xa3, 0xff),
    ],
    gizmo_hot: Color32::from_rgb(0xff, 0xe0, 0x66),
};

/// The viewport under the light palette: the same resin and marks, a pale plate, darker
/// lines and handles so they still read on it.
pub(super) const LIGHT: Scene = Scene {
    object: Color32::from_rgb(0x5a, 0x8a, 0xa0),
    support: Color32::from_rgb(0xa2, 0xa8, 0xb0),
    selected: Color32::from_rgb(0xf0, 0xa4, 0x3a),
    unsound: Color32::from_rgb(0xc9, 0x56, 0x4c),
    painted: Color32::from_rgb(0x3f, 0x9e, 0x7c),
    blocked: Color32::from_rgb(0x96, 0x58, 0xc8),
    fault: Color32::from_rgb(0x6e, 0x9a, 0x00),
    fault_rest: Color32::from_rgb(0xa9, 0xbd, 0x63),
    fault_picked: Color32::from_rgb(0xe0, 0x3a, 0xae),
    overhang: Color32::from_rgb(0xd9, 0x48, 0x1f),
    outside: Color32::from_rgb(0xd4, 0x2a, 0x2a),
    grid_minor: Color32::from_rgb(0xd6, 0xdc, 0xe3),
    grid_major: Color32::from_rgb(0xb3, 0xbc, 0xc7),
    plate_border: Color32::from_rgba_unmultiplied_const(0x7f, 0x8a, 0x98, 0xb3),
    volume: Color32::from_rgba_unmultiplied_const(0x7f, 0x8a, 0x98, 0x59),
    section_frame: Color32::from_rgb(0x5c, 0x65, 0x71),
    plate: Color32::from_rgb(0xe6, 0xea, 0xef),
    shadow: Color32::from_black_alpha(0x40),
    label: Color32::from_rgba_premultiplied(0x4a, 0x51, 0x5b, 0xc8),
    bounds: Color32::from_rgb(0x44, 0x4c, 0x57),
    plate_axis: super::palette::LIGHT.axis,
    gizmo: [
        Color32::from_rgb(0xe0, 0x3a, 0x3a),
        Color32::from_rgb(0x2f, 0xa5, 0x3a),
        Color32::from_rgb(0x2a, 0x78, 0xe6),
    ],
    gizmo_hot: Color32::from_rgb(0xe0, 0xa0, 0x00),
    ..DARK
};
