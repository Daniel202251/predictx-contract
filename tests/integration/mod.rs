//! Workspace-level integration tests.
///
/// Note: the workspace root is a virtual Cargo workspace (no root crate), so
/// this module is not executed by `cargo test` yet. Keep shared integration
/// scenarios here as a reference, and move/duplicate them into a dedicated test
/// crate if/when we introduce one.

/// The 64-voter cap is a window-scoped limit, not a permanent freeze.
///
/// When a round reaches `MAX_VOTERS`, the voter roster is pruned and the
/// dedup markers are cleared for the next window, so additional addresses can
/// vote in a later window. This ensures a capped poll can always reach a
/// settlement path via community vote rather than being permanently
/// unsettleable.
///
/// Abuse model: an attacker can fill the roster with sybil addresses to
/// exhaust the 64-voter window. This is mitigated because the cap is
/// window-scoped: once the window closes, the roster is pruned and the
/// attacker must re-establish control in a fresh window, and honest voters
/// can always participate in a later window to reach settlement.
///
/// TODO: move this into a dedicated integration test crate once the
/// workspace introduces one. The scenario below is the reference case for
/// a capped poll still reaching a settlement path.
///
/// #### Scenario: capped poll still reaches settlement
///
/// 1. A fresh poll has an empty voter roster and zero votes.
/// 2. Sixty-four distinct addresses vote in the current window.
/// 3. The 65th distinct address is rejected with `MaxVotersReached` for
///    this window.
/// 4. The window closes, the roster is pruned, and the dedup markers
///    are cleared.
/// 5. A new window opens; the 65th address (or any other address)
///    can now vote, and the poll can reach settlement.
///
/// This module is intentionally empty of executable code for now.
