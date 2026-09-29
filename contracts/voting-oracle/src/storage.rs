use crate::DataKey;
use predictx_shared::{PredictXError, VoteChoice, VoteTally};
use soroban_sdk::{Address, Env, Vec};

// ── Admin registry storage ────────────────────────────────────────────────────

/// Read the registered admins, defaulting to an empty list.
pub fn read_admins(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&DataKey::AdminList)
        .unwrap_or(Vec::new(env))
}

/// Persist the registered admins.
pub fn write_admins(env: &Env, admins: &Vec<Address>) {
    env.storage().instance().set(&DataKey::AdminList, admins);
}

/// Whether `addr` is a registered admin.
pub fn is_admin(env: &Env, addr: &Address) -> bool {
    read_admins(env).contains(addr.clone())
}

/// Ensure `caller` is a registered admin, else `Unauthorized`.
pub fn require_admin(env: &Env, caller: &Address) -> Result<(), PredictXError> {
    if is_admin(env, caller) {
        Ok(())
    } else {
        Err(PredictXError::Unauthorized)
    }
}

// ── Vote tally storage ────────────────────────────────────────────────────────

/// Read the vote tally for a poll, if one has been stored yet.
///
/// Tally data lives in *temporary* storage: it is only needed during the
/// voting window (matching the tier guidance in the shared `DataKey` layout).
pub fn read_tally(env: &Env, poll_id: u64) -> Option<VoteTally> {
    env.storage().temporary().get(&DataKey::VoteTally(poll_id))
}

/// Store the vote tally for a poll.
pub fn write_tally(env: &Env, tally: &VoteTally) {
    env.storage()
        .temporary()
        .set(&DataKey::VoteTally(tally.poll_id), tally);
}

// ── Voter roster storage ─────────────────────────────────────────────────────

/// The maximum number of distinct voters recorded in a poll's roster.
///
/// # Cap policy
///
/// The roster is *window-scoped*: it is only consulted while the poll is
/// open for voting. Once the cap is reached, additional distinct addresses
/// are rejected with `MaxVotersReached`, but this is **not** a permanent
/// freeze:
///
/// * The roster is stored in *persistent* storage keyed by `poll_id`, so it
///   is naturally scoped to a single poll and never leaks across polls.
/// * `clear_voters` provides an explicit recovery path: an admin (or any
///   caller authorized by the voting module) can reset the roster, which
///   reopens the poll to new distinct voters without discarding the tally.
/// * Because the tally lives in temporary storage and the dedup marker
///   expires with the voting window, a reset roster cannot be used to
///   double-count existing votes — `has_voted` still gates each address.
///
/// # Abuse model
///
/// An attacker can exhaust the roster by submitting 64 distinct sybil
/// addresses. This is a griefing vector, not a theft vector: it only
/// prevents *new* distinct voters from joining. The documented recovery
/// path (`clear_voters` + continued tallying) ensures the poll can still
/// reach a settlement via community vote, and the tally itself is never
/// frozen by the cap.
pub const MAX_VOTERS: u32 = 64;

/// Read the persistent voter roster for a poll, defaulting to an empty list.
pub fn read_voters(env: &Env, poll_id: u64) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::Voters(poll_id))
        .unwrap_or(Vec::new(env))
}

/// Persist the voter roster for a poll.
pub fn write_voters(env: &Env, poll_id: u64, voters: &Vec<Address>) {
    env.storage()
        .persistent()
        .set(&DataKey::Voters(poll_id), voters);
}

/// Clear the persistent voter roster for a poll.
///
/// This is the recovery path for the [`MAX_VOTERS`] cap: once the roster is
/// full, calling this reopens the poll to new distinct voters. The tally and
/// per-voter dedup markers are untouched, so previously cast votes are still
/// counted and cannot be replayed.
pub fn clear_voters(env: &Env, poll_id: u64) {
    env.storage().persistent().remove(&DataKey::Voters(poll_id));
}

// ── Vote-dedup storage ────────────────────────────────────────────────────────

/// Whether `voter` has already cast a vote on `poll_id`.
pub fn has_voted(env: &Env, poll_id: u64, voter: &Address) -> bool {
    env.storage()
        .temporary()
        .get(&DataKey::HasVoted(poll_id, voter.clone()))
        .unwrap_or(false)
}

/// Record that `voter` cast a vote on `poll_id`.
///
/// The marker lives in *temporary* storage so it expires with the tally when
/// the voting window closes.
pub fn write_voted(env: &Env, poll_id: u64, voter: &Address) {
    env.storage()
        .temporary()
        .set(&DataKey::HasVoted(poll_id, voter.clone()), &true);
}

// ── Voter reward storage ──────────────────────────────────────────────────────

/// The choice `voter` recorded on `poll_id`, if they voted.
pub fn read_vote_choice(env: &Env, poll_id: u64, voter: &Address) -> Option<VoteChoice> {
    env.storage()
        .persistent()
        .get(&DataKey::VoterChoice(poll_id, voter.clone()))
}

/// Persist the choice `voter` recorded on `poll_id`.
///
/// Stored in *persistent* storage (unlike the temporary tally and dedup marker)
/// because the choice must outlive the voting window so eligible voters can
/// still be identified when rewards are claimed.
pub fn write_vote_choice(env: &Env, poll_id: u64, voter: &Address, choice: VoteChoice) {
    env.storage()
        .persistent()
        .set(&DataKey::VoterChoice(poll_id, voter.clone()), &choice);
}

/// The voter reward reserve for `poll_id` (0 when unset).
pub fn read_reward_pool(env: &Env, poll_id: u64) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::RewardPool(poll_id))
        .unwrap_or(0)
}

/// Persist the voter reward reserve for `poll_id`.
pub fn write_reward_pool(env: &Env, poll_id: u64, amount: i128) {
    env.storage()
        .persistent()
        .set(&DataKey::RewardPool(poll_id), &amount);
}

/// Whether `voter` has already claimed their reward on `poll_id`.
pub fn has_claimed_reward(env: &Env, poll_id: u64, voter: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::RewardClaimed(poll_id, voter.clone()))
        .unwrap_or(false)
}

/// Record that `voter` claimed their reward on `poll_id`.
pub fn write_reward_claimed(env: &Env, poll_id: u64, voter: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::RewardClaimed(poll_id, voter.clone()), &true);
}
