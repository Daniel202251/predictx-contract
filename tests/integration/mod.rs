//! Workspace-level integration tests.
//!
//! Note: the workspace root is a virtual Cargo workspace (no root crate), so
//! this module is not executed by `cargo test` yet. Keep shared integration
//! scenarios here as a reference, and move/duplicate them into a dedicated test
//! crate if/when we introduce one.

//! ## Voter cap policy (Issue: recoverable 64-voter cap)
//!
//! The voting oracle enforces `MAX_VOTERS = 64` distinct voters per poll. The
//! cap is **window-scoped**, not a permanent freeze:
//!
//! * When the roster is full, additional distinct addresses are rejected with
//!   `MaxVotersReached` for the *current* window only.
//! * A window boundary (or an explicit admin reset of the roster) clears the
//!   persistent roster, so voting can be reopened and the poll remains
//!   settleable by community vote.
//! * Resolution never depends on the roster staying full: a capped poll still
//!   reaches a settlement path via the window-scoped reset.
//!
//! ### Abuse model for exhausting the roster
//!
//! An attacker can cheaply fill all 64 slots with sybil addresses to lock out
//! honest voters within a single window. This is bounded because:
//!
//! 1. The cap is per-window, so the lockout expires at the next window.
//! 2. The roster can be reset, restoring the ability to vote and settle.
//! 3. The temporary dedup marker expiring does not strand the poll, since the
//!    roster reset path is independent of the marker.
//!
//! The worst case is therefore a temporary denial of service for one window,
//! not a permanent freeze or an unsettleable poll.

/// Maximum distinct voters accepted per poll window.
///
/// Mirrors `contracts/voting-oracle/src/lib.rs` `MAX_VOTERS`.
pub const MAX_VOTERS: u32 = 64;

/// Outcome of attempting to register a voter against the roster.
#[derive(Debug, PartialEq, Eq)]
pub enum RegisterOutcome {
    /// Voter was added to the roster.
    Accepted,
    /// Voter was already present in the roster.
    AlreadyVoted,
    /// Roster is full for the current window; recoverable at the next window
    /// or via an explicit roster reset.
    MaxVotersReached,
}

/// Reference model of the persistent voter roster with a window-scoped cap.
///
/// This mirrors the on-chain storage semantics closely enough to exercise the
/// recovery path in tests without depending on the contract crate.
#[derive(Debug, Default)]
pub struct VoterRoster {
    voters: Vec<[u8; 32]>,
    window: u64,
}

impl VoterRoster {
    /// Creates a roster scoped to `window`.
    pub fn new(window: u64) -> Self {
        Self {
            voters: Vec::new(),
            window,
        }
    }

    /// Number of distinct voters currently recorded.
    pub fn len(&self) -> u32 {
        self.voters.len() as u32
    }

    /// Returns true when the roster has reached the cap for this window.
    pub fn is_full(&self) -> bool {
        self.len() >= MAX_VOTERS
    }

    /// Attempts to register `voter`.
    pub fn register(&mut self, voter: [u8; 32]) -> RegisterOutcome {
        if self.voters.contains(&voter) {
            return RegisterOutcome::AlreadyVoted;
        }
        if self.is_full() {
            return RegisterOutcome::MaxVotersReached;
        }
        self.voters.push(voter);
        RegisterOutcome::Accepted
    }

    /// Advances to `window`, clearing the roster so voting can be reopened.
    ///
    /// This is the defined recovery path: reaching the cap cannot leave a poll
    /// permanently unsettleable by community vote.
    pub fn advance_window(&mut self, window: u64) {
        self.window = window;
        self.voters.clear();
    }

    /// Explicit admin reset of the roster for the current window.
    pub fn reset(&mut self) {
        self.voters.clear();
    }
}

fn voter(seed: u8) -> [u8; 32] {
    [seed; 32]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_rejects_new_voters_but_is_not_permanent() {
        let mut roster = VoterRoster::new(0);

        for i in 0..MAX_VOTERS {
            let seed = (i % 256) as u8;
            assert_eq!(roster.register(voter(seed)), RegisterOutcome::Accepted);
        }
        assert!(roster.is_full());

        // A fresh address is rejected while the window is full.
        assert_eq!(
            roster.register(voter(200)),
            RegisterOutcome::MaxVotersReached
        );

        // Recovery: advancing the window clears the roster.
        roster.advance_window(1);
        assert!(!roster.is_full());
        assert_eq!(roster.register(voter(200)), RegisterOutcome::Accepted);
    }

    #[test]
    fn capped_poll_still_reaches_settlement_path() {
        let mut roster = VoterRoster::new(0);
        for i in 0..MAX_VOTERS {
            let seed = (i % 256) as u8;
            assert_eq!(roster.register(voter(seed)), RegisterOutcome::Accepted);
        }

        // The cap is reached, but the poll is not permanently frozen: the
        // window-scoped reset restores a settleable electorate.
        assert!(roster.is_full());
        roster.advance_window(1);
        assert_eq!(roster.register(voter(201)), RegisterOutcome::Accepted);
        assert!(roster.len() >= 1, "poll has a live electorate to settle");
    }

    #[test]
    fn explicit_reset_recovers_a_capped_roster() {
        let mut roster = VoterRoster::new(0);
        for i in 0..MAX_VOTERS {
            let seed = (i % 256) as u8;
            assert_eq!(roster.register(voter(seed)), RegisterOutcome::Accepted);
        }
        assert_eq!(
            roster.register(voter(202)),
            RegisterOutcome::MaxVotersReached
        );

        roster.reset();
        assert!(!roster.is_full());
        assert_eq!(roster.register(voter(202)), RegisterOutcome::Accepted);
    }

    #[test]
    fn duplicate_voter_does_not_consume_a_slot() {
        let mut roster = VoterRoster::new(0);
        assert_eq!(roster.register(voter(1)), RegisterOutcome::Accepted);
        assert_eq!(roster.register(voter(1)), RegisterOutcome::AlreadyVoted);
        assert_eq!(roster.len(), 1);
    }
}
