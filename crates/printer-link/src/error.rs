use net_prusalink::PrusaLinkError;
use net_sdcp::SdcpError;

/// Why an errand to a printer ended badly, in the words of the protocol that failed.
#[derive(Debug, thiserror::Error)]
pub enum SendError {
    #[error(transparent)]
    Sdcp(#[from] SdcpError),

    #[error(transparent)]
    Prusa(#[from] PrusaLinkError),
}

impl SendError {
    /// Whether the errand stopped because it was asked to rather than because it failed.
    pub fn is_cancelled(&self) -> bool {
        matches!(
            self,
            Self::Sdcp(SdcpError::Cancelled) | Self::Prusa(PrusaLinkError::Cancelled)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelling_is_told_from_failing_whichever_protocol_reports_it() {
        assert!(SendError::from(SdcpError::Cancelled).is_cancelled());
        assert!(SendError::from(PrusaLinkError::Cancelled).is_cancelled());
        let refused = SendError::from(PrusaLinkError::Unauthorized {
            host: "sl1.local".to_owned(),
        });
        assert!(!refused.is_cancelled());
        assert!(refused.to_string().contains("sl1.local"), "got {refused}");
    }
}
