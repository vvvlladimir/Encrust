use std::collections::BTreeMap;
use std::net::IpAddr;
use std::time::Duration;

use net_prusalink::Link;
use net_sdcp::Printer;

/// What one scan came back with, from both protocols at once.
#[derive(Debug, Clone, Default)]
pub struct Found {
    /// Boards that answered the broadcast or their address, one each.
    pub boards: Vec<Printer>,
    /// What each Prusa machine that answered calls itself, keyed by its host.
    pub reached: BTreeMap<String, String>,
}

/// Asks every board on the segment and each of `addresses` to introduce itself for
/// `window`, and each Prusa machine in `links` what it is.
///
/// A board answers a broadcast; a Prusa machine is set up rather than found (ADR 0153), so
/// only the machines already described are asked. A failed broadcast finds no boards.
pub fn scan(window: Duration, addresses: &[IpAddr], links: &[Link]) -> Found {
    let mut boards = net_sdcp::discover(window).unwrap_or_else(|error| {
        tracing::debug!(%error, "the discovery broadcast failed");
        Vec::new()
    });
    for &address in addresses {
        if boards.iter().any(|board| board.address == address) {
            continue;
        }
        match net_sdcp::probe(address, window) {
            Ok(Some(board)) => boards.push(board),
            Ok(None) => tracing::debug!(%address, "no board answered"),
            Err(error) => tracing::debug!(%address, %error, "a board could not be asked"),
        }
    }
    let mut reached = BTreeMap::new();
    for link in links {
        match net_prusalink::probe(link) {
            Ok(version) => {
                reached.insert(link.host.clone(), version.text);
            }
            Err(error) => {
                tracing::debug!(host = %link.host, %error, "a Prusa machine did not answer");
            }
        }
    }
    Found { boards, reached }
}
