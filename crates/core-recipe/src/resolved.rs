//! A step's resolved settings read by key, as each front end's runner reads them.

use std::collections::BTreeMap;

use core_geometry::Scalar;

use crate::decide::Resolved;

/// A step's settings with every expression worked out, by key.
pub type ResolvedSettings = BTreeMap<String, Resolved>;

pub fn number(settings: &ResolvedSettings, key: &str) -> Option<Scalar> {
    match settings.get(key)? {
        Resolved::Number(number) => Some(*number),
        _ => None,
    }
}

pub fn flag(settings: &ResolvedSettings, key: &str) -> Option<bool> {
    match settings.get(key)? {
        Resolved::Bool(flag) => Some(*flag),
        _ => None,
    }
}

/// A word or a text, which a setting such as a profile id may be written as.
pub fn text<'a>(settings: &'a ResolvedSettings, key: &str) -> Option<&'a str> {
    match settings.get(key)? {
        Resolved::Word(text) | Resolved::Text(text) => Some(text),
        _ => None,
    }
}

/// `[x, y, z]` fractions of the placed box; none where the key is not points.
pub fn points<'a>(settings: &'a ResolvedSettings, key: &str) -> &'a [[Scalar; 3]] {
    match settings.get(key) {
        Some(Resolved::Points(points)) => points,
        _ => &[],
    }
}
