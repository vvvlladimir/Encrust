//! The `.cfgx` machine configuration: JSON, the resin in the material's current slice.

use std::path::Path;

use serde_json::Value;

use super::{ForeignResin, named};
use crate::{
    Compensation, MaterialProfile, PriceUnit, PrinterTuning, ProfileError, ResinDetails,
    SecondStage, WaitMode, Waits, stated_distance_mm, stated_pwm, stated_speed_mm_min,
    stated_wait_s,
};

/// The currencies `resin.resinunit` indexes, in the order the format lists them.
const CURRENCIES: [&str; 38] = [
    "¥", "$", "€", "₤", "руб", "៛", "₪", "₴", "zł", "￥", "NT", "₩", "₮", "〒", "R$", "৲৳", "Rp",
    "Sk", "R", "Q", "P", "₱", "₦", "Lm", "kr", "₭", "₲", "Rs", "Ft", "ƒ", "₫", "฿", "₡", "₵", "Br",
    "Bs", "﷼", "¤",
];

pub(super) fn read(text: &str, path: &Path) -> Result<ForeignResin, ProfileError> {
    let root: Value = serde_json::from_str(text).map_err(|source| ProfileError::Json {
        path: path.to_owned(),
        source,
    })?;
    let missing = |key| ProfileError::MissingSetting {
        path: path.to_owned(),
        key,
    };
    let slice = Slice(current_slice(&root).ok_or_else(|| missing("material"))?);
    let exposure_s = slice
        .number("print", "normalexposuretime")
        .ok_or_else(|| missing("print.normalexposuretime"))?;
    let resin = MaterialProfile {
        name: slice.text("resin", "name").unwrap_or_default(),
        density_g_cm3: slice
            .number("resin", "resindensity")
            .filter(|density| *density > 0.0)
            .unwrap_or(MaterialProfile::default().density_g_cm3),
        details: details(&slice),
        exposure_s,
        ..MaterialProfile::default()
    };
    let printer = Slice(root.get("printerinfo").unwrap_or(&Value::Null));
    Ok(ForeignResin {
        resin: resin.with_tuning(&tuning(&slice)),
        machine: printer.text_at("machinetype"),
        panel_px: printer
            .count_at("resolutionx")
            .zip(printer.count_at("resolutiony"))
            .map(|(width, height)| [width, height]),
    })
}

/// The slice the material in use prints with. A stale index names the first one.
fn current_slice(root: &Value) -> Option<&Value> {
    let index = |value: &Value, key| value.get(key).and_then(Value::as_u64).unwrap_or(0);
    let material = pick(root.get("material")?, index(root, "current_material"))?;
    pick(material.get("slice")?, index(material, "current_slice"))
}

/// Item `at` of a list, or its first where `at` is past the end; a lone object as itself.
fn pick(list: &Value, at: u64) -> Option<&Value> {
    match list {
        Value::Array(items) => usize::try_from(at)
            .ok()
            .and_then(|at| items.get(at))
            .or_else(|| items.first()),
        other => Some(other),
    }
}

fn tuning(slice: &Slice) -> PrinterTuning {
    let print = |key| slice.number("print", key);
    let pwm = |key| slice.count("advanced", key).and_then(pwm_of);
    let lift_distance_mm = print("normallayerliftheight").and_then(stated_distance_mm);
    let stage = second_stage(slice);
    PrinterTuning {
        layer_height_mm: print("layerheight").filter(|height| *height > 0.0),
        bottom_exposure_s: print("bottomlayerexposuretime"),
        bottom_layers: slice.count("print", "bottomlayercount"),
        transition_layers: slice
            .count("print", "transitionlayercount")
            .and_then(|count| u16::try_from(count).ok()),
        light_off_delay_s: print("lightofftime").and_then(stated_wait_s),
        bottom_light_off_delay_s: print("bottomlightofftime").and_then(stated_wait_s),
        light_pwm: pwm("normallightintensitypwm"),
        bottom_light_pwm: pwm("bottomlightintensitypwm"),
        lift_distance_mm,
        lift_speed_mm_min: print("normallayerliftspeed").and_then(stated_speed_mm_min),
        // The plate comes back down as far as both stages lifted it, the last stretch slower.
        retract_distance_mm: lift_distance_mm
            .map(|lift| (lift + stage.lift_distance_mm - stage.retract_distance_mm).max(0.0)),
        retract_speed_mm_min: print("normallayerdropspeed").and_then(stated_speed_mm_min),
        bottom_lift_distance_mm: print("bottomlayerliftheight").and_then(stated_distance_mm),
        bottom_lift_speed_mm_min: print("bottomlayerliftspeed").and_then(stated_speed_mm_min),
        bottom_retract_speed_mm_min: print("bottomlayerdropspeed").and_then(stated_speed_mm_min),
        second_stage: Some(stage),
        waits: Some(waits(slice)),
        compensation: Some(compensation(slice)),
        ..PrinterTuning::default()
    }
}

/// The second stage of each peel, a stage the file leaves out moving nowhere.
fn second_stage(slice: &Slice) -> SecondStage {
    let distance = |key| {
        slice
            .number("print", key)
            .and_then(stated_distance_mm)
            .unwrap_or(0.0)
    };
    let speed = |key| {
        slice
            .number("print", key)
            .and_then(stated_speed_mm_min)
            .unwrap_or(0.0)
    };
    SecondStage {
        lift_distance_mm: distance("normallayerliftheight2"),
        lift_speed_mm_min: speed("normallayerliftspeed2"),
        retract_distance_mm: distance("normallayerdropheight2"),
        retract_speed_mm_min: speed("normallayerdropspeed2"),
        bottom_lift_distance_mm: distance("bottomlayerliftheight2"),
        bottom_lift_speed_mm_min: speed("bottomlayerliftspeed2"),
        bottom_retract_distance_mm: distance("bottomlayerdropheight2"),
        bottom_retract_speed_mm_min: speed("bottomlayerdropspeed2"),
    }
}

fn pwm_of(value: u32) -> Option<u8> {
    stated_pwm(u16::try_from(value).ok()?)
}

/// The common rests, the only set a resin holds; the bottom block's are left behind.
fn waits(slice: &Slice) -> Waits {
    let rest = |key| {
        slice
            .number("print", key)
            .and_then(stated_wait_s)
            .unwrap_or(0.0)
    };
    Waits {
        mode: match slice.count("print", "printwaitmode") {
            Some(0) => WaitMode::Rest,
            _ => WaitMode::LightOff,
        },
        before_lift_s: rest("normalresttimebeforelift"),
        after_lift_s: rest("normalresttimeafterlift"),
        after_retract_s: rest("normalresttimeafterretract"),
    }
}

fn compensation(slice: &Slice) -> Compensation {
    let neutral = Compensation::default();
    let advanced = |key, or: f32| slice.number("advanced", key).unwrap_or(or);
    let on = |key| slice.flag("advanced", key).unwrap_or(false);
    Compensation {
        shrinkage_on: on("enable_scale_compensation"),
        tolerance_on: on("binnerouttercircleoffsetenable"),
        bottom_tolerance_on: on("bbottominnerouttercircleoffsetenable"),
        shrink_x_pct: advanced("compensation_x", neutral.shrink_x_pct),
        shrink_y_pct: advanced("compensation_y", neutral.shrink_y_pct),
        shrink_z_pct: advanced("compensation_z", neutral.shrink_z_pct),
        hole_offset_mm: advanced("innercircleoffset", 0.0),
        outer_offset_mm: advanced("outtercircleoffset", 0.0),
        bottom_hole_offset_mm: advanced("bottominnercircleoffset", 0.0),
        bottom_outer_offset_mm: advanced("bottomouttercircleoffset", 0.0),
        parity_offset_mm: advanced("circleparitydifference", 0.0),
        layer_time_s: if on("bprinttimecompensationenable") {
            advanced("layerprinttime", 0.0)
        } else {
            0.0
        },
    }
}

fn details(slice: &Slice) -> ResinDetails {
    let neutral = ResinDetails::default();
    let currency = slice
        .count("resin", "resinunit")
        .and_then(|index| CURRENCIES.get(usize::try_from(index).ok()?));
    // The window shows 0 as a price per kilogram, whatever the field's own list says.
    let price_per = match slice.count("resin", "resinmassunit") {
        Some(1) => PriceUnit::Litre,
        _ => PriceUnit::Kilogram,
    };
    let per = slice
        .number("resin", "resincostmeasure")
        .filter(|per| *per > 0.0)
        .unwrap_or(1.0);
    ResinDetails {
        kind: slice.text("resin", "resintype").unwrap_or_default(),
        color: slice
            .count("resin", "color")
            .map_or(neutral.color, |rgb| [16, 8, 0].map(|at| (rgb >> at) as u8)),
        price: slice
            .number("resin", "resincost")
            .filter(|price| *price > 0.0)
            .map_or(0.0, |price| price / per),
        currency: currency.map_or(neutral.currency, |sign| (*sign).to_owned()),
        price_per,
    }
}

/// A section of the file, each of whose settings is an object carrying `currentvalue`.
struct Slice<'a>(&'a Value);

impl Slice<'_> {
    fn value(&self, section: &str, key: &str) -> Option<&Value> {
        self.0.get(section)?.get(key)?.get("currentvalue")
    }

    fn value_at(&self, key: &str) -> Option<&Value> {
        self.0.get(key)?.get("currentvalue")
    }

    fn number(&self, section: &str, key: &str) -> Option<f32> {
        let value = self.value(section, key)?.as_f64()? as f32;
        value.is_finite().then_some(value)
    }

    fn count(&self, section: &str, key: &str) -> Option<u32> {
        u32::try_from(self.value(section, key)?.as_u64()?).ok()
    }

    fn count_at(&self, key: &str) -> Option<u32> {
        u32::try_from(self.value_at(key)?.as_u64()?).ok()
    }

    fn flag(&self, section: &str, key: &str) -> Option<bool> {
        self.value(section, key)?.as_bool()
    }

    fn text(&self, section: &str, key: &str) -> Option<String> {
        named(self.value(section, key)?.as_str()?)
    }

    fn text_at(&self, key: &str) -> Option<String> {
        named(self.value_at(key)?.as_str()?)
    }
}
