use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

/// The highest share a bar has shown, so counters read apart by the window never step it back.
#[derive(Debug, Default)]
pub struct Highest(AtomicU32);

impl Highest {
    /// `share` or the highest shown before it, whichever is greater.
    pub fn keep(&self, share: f32) -> f32 {
        // A non-negative f32's bits order as its value does.
        let bits = share.max(0.0).to_bits();
        f32::from_bits(self.0.fetch_max(bits, Relaxed).max(bits))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lower_share_read_later_keeps_the_higher() {
        let highest = Highest::default();
        assert_eq!(highest.keep(0.4), 0.4);
        assert_eq!(
            highest.keep(0.1),
            0.4,
            "a mixed read of 0.1 after 0.4 holds 0.4"
        );
        assert_eq!(highest.keep(0.7), 0.7);
    }
}
