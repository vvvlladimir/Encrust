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

const PALETTES: [(&str, &Palette); 2] = [("dark", &palette::DARK), ("light", &palette::LIGHT)];

#[test]
fn every_text_colour_meets_aa_on_the_panel() {
    for (name, colors) in PALETTES {
        // 4.5:1 is WCAG AA for body text, 3:1 for the large or incidental text the low
        // tone is reserved for.
        assert!(
            contrast(colors.text_high, colors.panel) >= 4.5,
            "{name} high"
        );
        assert!(contrast(colors.text_mid, colors.panel) >= 4.5, "{name} mid");
        assert!(contrast(colors.text_low, colors.panel) >= 3.0, "{name} low");
    }
}

/// The accent as a fill is a control (3:1); as text it is the soft tone (4.5:1).
#[test]
fn the_accent_carries_its_own_label() {
    for (name, colors) in PALETTES {
        assert!(contrast(colors.accent, colors.panel) >= 3.0, "{name} fill");
        assert!(
            contrast(colors.accent_soft, colors.panel) >= 4.5,
            "{name} text"
        );
        assert!(
            contrast(colors.on_accent, colors.accent) >= 4.5,
            "{name} label"
        );
    }
}

#[test]
fn every_wash_keeps_its_colour_readable_on_the_panel() {
    for (theme, colors) in PALETTES {
        for (name, ink, wash) in [
            ("warn", colors.warn, colors.warn_wash),
            ("ok", colors.ok, colors.ok_wash),
            ("danger", colors.danger, colors.danger_wash),
            ("accent", colors.accent_soft, colors.accent_wash),
            ("picked", colors.text_high, colors.picked_wash),
        ] {
            let ground = colors.panel.blend(wash);
            let ratio = contrast(ink, ground);
            assert!(
                ratio >= 4.5,
                "{theme} {name} on its wash is {ratio:.2}:1, AA is 4.5:1"
            );
        }
    }
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
    for scene in [&scene::DARK, &scene::LIGHT] {
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
}

/// The amber of a picked model once faded into the pale plate. 3:1 is WCAG's floor for
/// a graphic; the same order of lightness keeps each role recognisable across palettes.
#[test]
fn a_model_stands_off_the_plate_in_one_order_under_both_palettes() {
    for (theme, scene) in [("dark", &scene::DARK), ("light", &scene::LIGHT)] {
        let surfaces = [
            ("selected", scene.selected),
            ("object", scene.object),
            ("unsound", scene.unsound),
        ];
        for (name, surface) in surfaces {
            let ratio = contrast(surface, scene.plate);
            assert!(
                ratio >= 3.0,
                "{theme} {name} on the plate is {ratio:.2}:1, want 3:1"
            );
        }
        assert!(
            surfaces
                .windows(2)
                .all(|pair| luminance(pair[0].1) > luminance(pair[1].1)),
            "{theme}: a picked model is the lightest, an unsound one the darkest"
        );
    }
}

/// Dark surfaces climb towards the pointer; light ones fall from a white panel.
#[test]
fn the_surfaces_are_ordered_from_sunken_to_hover() {
    let ladder = |colors: &Palette| {
        [
            colors.sunken,
            colors.base,
            colors.panel,
            colors.raised,
            colors.hover,
            colors.active,
        ]
        .map(luminance)
    };
    let dark = ladder(&palette::DARK);
    assert!(
        dark.windows(2).all(|pair| pair[0] < pair[1]),
        "dark climbs: {dark:?}"
    );
    let light = ladder(&palette::LIGHT);
    assert!(
        light[..3].windows(2).all(|pair| pair[0] < pair[1]),
        "light climbs to the panel"
    );
    assert!(
        light[2..].windows(2).all(|pair| pair[0] > pair[1]),
        "light falls from the panel"
    );
}
