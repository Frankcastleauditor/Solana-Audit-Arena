pub const CONFIG_SEED: &[u8] = b"config";
pub const VAULT_SEED: &[u8] = b"vault";
pub const AGENT_SEED: &[u8] = b"agent";
pub const RECEIPT_SEED: &[u8] = b"receipt";

/// Max byte length of the agent HTTP endpoint (https://...).
pub const MAX_ENDPOINT_LEN: usize = 200;
/// Max byte length of the agent metadata URI (https:// or ipfs://).
pub const MAX_METADATA_URI_LEN: usize = 200;

/// Protocol fee is capped at 10%. The cap is enforced on every write of fee_bps.
pub const MAX_FEE_BPS: u16 = 1_000;
pub const BPS_DENOMINATOR: u64 = 10_000;

/// Allowed window between payment creation and expiry. A payment can only be
/// settled strictly before expiry and only refunded at or after expiry.
pub const MIN_PAYMENT_WINDOW_SECS: i64 = 60;
pub const MAX_PAYMENT_WINDOW_SECS: i64 = 86_400;

/// Feedback score range (inclusive).
pub const MIN_SCORE: u8 = 1;
pub const MAX_SCORE: u8 = 5;
