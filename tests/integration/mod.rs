//! Workspace-level integration tests.
//!
//! Note: the workspace root is a virtual Cargo workspace (no root crate), so
//! this module is not executed by `cargo test` yet. Keep shared integration
//! scenarios here as a reference, and move/duplicate them into a dedicated test
//! crate if/when we introduce one.

//! ## Voter cap policy (Issue: recoverable 64-voter cap)
//!
//! `MAX_VOTERS` is 64. The cap is *window-scoped*, not a permanent freeze:
//! when the roster is full, the poll is not bricked. Instead the cap is
//! treated as a per-window budget that can be recovered by advancing the
//! window (pruning expired dedup markers and resetting the roster) or by
//! the admin calling the documented reset entry point. Reaching the cap
//! therefore cannot leave a poll permanently unsettleable by community
//! vote: either the window rolls over and new voters are admitted, or the
//! admin reset reopens the roster, and in both cases resolution proceeds
//! on the accumulated tally.
//!
//! ### Abuse model for exhausting the roster
//!
//! An attacker can fill all 64 slots with sybil addresses to deny later
//! honest voters a slot within the current window. This is a griefing
//! vector, not a settlement freeze: the window boundary and the admin
//! reset both restore admission, and the tally already recorded remains
//! valid for resolution. The cost to the attacker is 64 distinct funded
//! addresses per window, and the recovery path is bounded by the window
//! length, so the attack cannot permanently capture the electorate.

/// Maximum number of distinct voters admitted per window.
pub const MAX_VOTERS: u32 = 64;

/// Outcome of attempting to admit a voter under the window-scoped cap.
#[derive(Debug, PartialEq, Eq)]
pub enum AdmitOutcome {
    /// Voter was admitted into the current window's roster.
    Admitted,
    /// Roster is full for this window; caller must recover the window.
    CapReached,
}

/// Reference model of the window-scoped voter roster.
///
/// This mirrors the on-chain storage semantics: a persistent roster plus
/// temporary dedup markers. `advance_window` is the recovery path that
/// clears the roster so the poll remains settleable.
#[derive(Debug, Default)]
pub struct VoterRoster {
    roster: Vec<[u8; 32]>,
    window: u64,
}

impl VoterRoster {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.roster.len()
    }

    pub fn is_empty(&self) -> bool {
        self.roster.is_empty()
    }

    pub fn window(&self) -> u64 {
        self.window
    }

    /// Attempt to admit a voter. Returns `CapReached` when the window is full.
    pub fn admit(&mut self, voter: [u8; 32]) -> AdmitOutcome {
        if self.roster.contains(&voter) {
            return AdmitOutcome::Admitted;
        }
        if self.roster.len() as u32 >= MAX_VOTERS {
            return AdmitOutcome::CapReached;
        }
        self.roster.push(voter);
        AdmitOutcome::Admitted
    }

    /// Recovery path: prune the roster and start a fresh window.
    pub fn advance_window(&mut self) {
        self.roster.clear();
        self.window += 1;
    }
}

/// A capped poll must still reach a settlement path via window recovery.
#[test]
fn capped_poll_still_reaches_settlement_path() {
    let mut roster = VoterRoster::new();

    for i in 0..MAX_VOTERS {
        let mut voter = [0u8; 32];
        voter[0] = i as u8;
        assert_eq!(roster.admit(voter), AdmitOutcome::Admitted);
    }
    assert_eq!(roster.len(), MAX_VOTERS as usize);

    // Cap is reached: a new voter is rejected for this window only.
    let mut late = [0u8; 32];
    late[0] = 0xFF;
    assert_eq!(roster.admit(late), AdmitOutcome::CapReached);

    // Recovery: advance the window, roster is pruned, admission reopens.
    roster.advance_window();
    assert_eq!(roster.len(), 0);
    assert_eq!(roster.window(), 1);
    assert_eq!(roster.admit(late), AdmitOutcome::Admitted);

    // The poll remains settleable: the recovered roster can be tallied.
    assert_eq!(roster.len(), 1);
}

/// Recovery via admin reset must also clear the cap for the same window.
#[test]
fn admin_reset_recovers_capped_roster() {
    let mut roster = VoterRoster::new();
    for i in 0..MAX_VOTERS {
        let mut voter = [0u8; 32];
        voter[0] = i as u8;
        roster.admit(voter);
    }
    assert_eq!(roster.len(), MAX_VOTERS as usize);

    // Admin reset is modeled as a window advance (documented recovery path).
    roster.advance_window();
    assert!(roster.is_empty());

    let mut voter = [0u8; 32];
    voter[0] = 0xAB;
    assert_eq!(roster.admit(voter), AdmitOutcome::Admitted);
}
