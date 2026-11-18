//! Plans the engine's tests place by.

use printer_profiles::{Catalogue, SupportProfile};

use crate::SupportPlan;

/// The first shipped automatic profile, sizing each support among its shapes.
pub(crate) fn sized() -> SupportPlan {
    let catalogue = Catalogue::bundled().expect("the bundled profiles load");
    let auto = catalogue
        .autos()
        .next()
        .expect("an automatic profile ships")
        .profile
        .clone();
    SupportPlan::sized(auto, &catalogue).expect("its shapes ship")
}

/// The shipped light profile, one shape for every support.
pub(crate) fn light() -> SupportPlan {
    SupportPlan::of(SupportProfile::shipped("light").expect("light ships"))
}
