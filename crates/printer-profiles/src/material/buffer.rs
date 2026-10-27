use serde::{Deserialize, Serialize};

use crate::{MaterialProfile, PrinterProfile};

/// A slow stretch of travel the firmware runs itself, then rests before the exposure: the
/// HALOT line's buffer, which only `.cxdlpv4` carries. See ADR 0371.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Buffer {
    pub enabled: bool,
    pub height_mm: f32,
    pub speed_mm_min: f32,
    pub light_off_delay_s: f32,
}

impl MaterialProfile {
    /// The buffer a file for `printer` states: the resin's own over the machine's default,
    /// and none for a machine whose firmware has no buffer.
    pub fn buffer_on(&self, printer: &PrinterProfile) -> Option<Buffer> {
        printer.buffer.map(|default| self.buffer.unwrap_or(default))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MACHINE: Buffer = Buffer {
        enabled: true,
        height_mm: 5.0,
        speed_mm_min: 120.0,
        light_off_delay_s: 6.0,
    };

    #[test]
    fn a_machine_without_a_buffer_states_none_whatever_the_resin_says() {
        let resin = MaterialProfile {
            buffer: Some(MACHINE),
            ..MaterialProfile::default()
        };
        assert_eq!(resin.buffer_on(&PrinterProfile::default()), None);
    }

    #[test]
    fn a_resin_saying_nothing_takes_the_machines_buffer_and_its_own_wins() {
        let printer = PrinterProfile {
            buffer: Some(MACHINE),
            ..PrinterProfile::default()
        };
        assert_eq!(
            MaterialProfile::default().buffer_on(&printer),
            Some(MACHINE)
        );
        let off = Buffer {
            enabled: false,
            ..MACHINE
        };
        let resin = MaterialProfile {
            buffer: Some(off),
            ..MaterialProfile::default()
        };
        assert_eq!(resin.buffer_on(&printer), Some(off));
    }
}
