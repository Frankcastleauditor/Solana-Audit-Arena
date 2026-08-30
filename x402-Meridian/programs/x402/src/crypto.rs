use tiny_keccak::{Hasher, Keccak};

use crate::state::Witness;

const WITNESS_DOMAIN_TAG: &[u8] = b"solana-x402:Exact:Witness";

pub const WITNESS_TYPE_STRING: &str = "Witness(address to,uint256 validAfter)";

fn keccak256(chunks: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    for c in chunks {
        hasher.update(c);
    }
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    out
}

pub fn hash_witness(witness: &Witness) -> [u8; 32] {
    keccak256(&[
        WITNESS_DOMAIN_TAG,
        witness.to.as_ref(),
        &witness.valid_after.to_le_bytes(),
    ])
}
