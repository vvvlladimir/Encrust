//! `profiles new-resin`: a resin from the numbers on its bottle or its maker's page.

use anyhow::Result;
use printer_profiles::MaterialProfile;

use crate::commands::GlobalArgs;
use crate::{json, profiles};

#[derive(clap::Args, Debug)]
pub struct NewResinArgs {
    /// The printer it is set up on, by id.
    #[arg(long, value_name = "ID")]
    pub printer: String,
    /// What it is called, as on its bottle.
    #[arg(long)]
    pub name: String,
    /// Exposure of a normal layer, s.
    #[arg(long, value_name = "S", value_parser = positive)]
    pub exposure: f32,
    /// Exposure of a bottom layer, s.
    #[arg(long, value_name = "S", value_parser = positive)]
    pub bottom_exposure: f32,
    /// How many bottom layers.
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..))]
    pub bottom_layers: u32,
    /// Layer height the numbers are for, mm; 0.05 when absent.
    #[arg(long, value_name = "MM", value_parser = positive)]
    pub layer_height: Option<f32>,
    /// Wait with the light off before each layer, s.
    #[arg(long, value_name = "S")]
    pub light_off_delay: Option<f32>,
    /// Lift distance, mm.
    #[arg(long, value_name = "MM", value_parser = positive)]
    pub lift_distance: Option<f32>,
    /// Lift speed, mm/min.
    #[arg(long, value_name = "MM_MIN", value_parser = positive)]
    pub lift_speed: Option<f32>,
    /// Retract speed, mm/min.
    #[arg(long, value_name = "MM_MIN", value_parser = positive)]
    pub retract_speed: Option<f32>,
    /// Layers over which the exposure fades from the bottom one to the normal one.
    #[arg(long, value_name = "N")]
    pub transition_layers: Option<u16>,
    /// Density, g/cm³, for the weight a print is reported at.
    #[arg(long, value_name = "G_CM3", value_parser = positive)]
    pub density: Option<f32>,
}

impl NewResinArgs {
    /// The resin these numbers describe, the stock ones where none is given.
    fn resin(&self) -> MaterialProfile {
        let stock = MaterialProfile::default();
        MaterialProfile {
            name: self.name.clone(),
            exposure_s: self.exposure,
            bottom_exposure_s: self.bottom_exposure,
            bottom_layers: self.bottom_layers,
            layer_height_mm: self.layer_height.unwrap_or(stock.layer_height_mm),
            light_off_delay_s: self.light_off_delay.unwrap_or(stock.light_off_delay_s),
            lift_distance_mm: self.lift_distance.unwrap_or(stock.lift_distance_mm),
            lift_speed_mm_min: self.lift_speed.unwrap_or(stock.lift_speed_mm_min),
            retract_speed_mm_min: self.retract_speed.unwrap_or(stock.retract_speed_mm_min),
            transition_layers: self.transition_layers.unwrap_or(stock.transition_layers),
            density_g_cm3: self.density.unwrap_or(stock.density_g_cm3),
            ..stock
        }
    }

    pub fn run(&self, global: &GlobalArgs) -> Result<()> {
        let (id, advised) = profiles::new_resin(self.resin(), &self.printer)?;
        if global.json {
            return json::print(&serde_json::json!({
                "id": id,
                "kind": "resin",
                "printer": self.printer,
                "advised_against": advised,
            }));
        }
        if global.talks() {
            println!(
                "resin {} kept as {id}, set up on {}",
                self.name, self.printer
            );
            for warning in &advised {
                println!("  {warning}");
            }
        }
        Ok(())
    }
}

fn positive(text: &str) -> Result<f32, String> {
    match text.parse::<f32>() {
        Ok(value) if value > 0.0 && value.is_finite() => Ok(value),
        _ => Err(format!("{text} is not a number above zero")),
    }
}
