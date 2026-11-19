//! A clock all paired devices agree on.
//!
//! Every shared time (when a Pomodoro started, when a detox ends) is a Unix time, and each
//! device works out what's left with its own clock. Even with automatic time on, two clocks can
//! be a few seconds apart, and the countdowns then differ by as much. So every announcement
//! carries the sender's own clock (`sent_at`), each device works out how far the others' clocks
//! are from its own, and they all follow the middle one (the median, counting their own). With
//! three or more devices one wrong clock is simply outvoted; two devices meet halfway.
//!
//! The offsets compare raw clocks, never corrected ones, so the devices don't chase each other.
//! The local network's delay (a few milliseconds) is ignored.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// A device not heard from for this long no longer counts
const FORGET_AFTER: Duration = Duration::from_secs(30);

/// How far the group's time is from this device's clock, in milliseconds
static OFFSET: AtomicI64 = AtomicI64::new(0);

/// This device's own clock, as a Unix time in milliseconds
pub fn system_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// The group's time, as a Unix time in milliseconds: the time to use for everything shared
pub fn now_ms() -> u64 {
    system_ms().saturating_add_signed(offset_ms())
}

/// How far the group's time is from this device's clock
pub fn offset_ms() -> i64 {
    OFFSET.load(Ordering::Relaxed)
}

/// How far the other devices' clocks are from this one
#[derive(Default)]
pub struct Clocks {
    peers: HashMap<u64, (i64, Instant)>,
}

impl Clocks {
    /// `device` said its clock read `sent_at` when this one read `received_at`
    pub fn heard(&mut self, device: u64, sent_at: u64, received_at: u64) {
        let offset = sent_at as i64 - received_at as i64;
        self.peers.insert(device, (offset, Instant::now()));
        self.update();
    }

    /// Forgets the devices that went quiet
    pub fn update(&mut self) {
        self.peers.retain(|_, (_, at)| at.elapsed() < FORGET_AFTER);
        let offsets = self.peers.values().map(|(offset, _)| *offset);
        OFFSET.store(median(offsets), Ordering::Relaxed);
    }

    /// Back to this device's own clock (unpaired, or syncing turned off)
    pub fn clear(&mut self) {
        self.peers.clear();
        OFFSET.store(0, Ordering::Relaxed);
    }
}

/// The middle of the offsets of the other devices and this one's (0); between the two middle
/// ones if they're an even number
fn median(others: impl Iterator<Item = i64>) -> i64 {
    let mut all: Vec<i64> = std::iter::once(0).chain(others).collect();
    all.sort_unstable();
    let n = all.len();
    if n % 2 == 1 { all[n / 2] } else { (all[n / 2 - 1] + all[n / 2]) / 2 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_clock_ahead_is_outvoted() {
        // Seen from the tablet, 4.5 s ahead: the other three are all behind it
        assert_eq!(median([-4500, -4510, -4490].into_iter()), -4495);
        // Seen from the PC: the laptop and the phone agree with it, the tablet doesn't
        assert_eq!(median([10, -5, 4500].into_iter()), 5);
    }

    #[test]
    fn two_devices_meet_halfway() {
        assert_eq!(median([4000].into_iter()), 2000);
        assert_eq!(median([-4000].into_iter()), -2000);
        // Alone: its own clock
        assert_eq!(median(std::iter::empty()), 0);
    }
}
