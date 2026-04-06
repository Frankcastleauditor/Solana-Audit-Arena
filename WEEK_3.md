# Week 3 — Zenon

**Category**: Token Launchpad / Bonding Curve AMM

**Difficulty**: 🟡 Intermediate

**Submission deadline**: Sunday April 13, 2026 — 23:59 UTC

**Commit Hash**: *(to be added on publish)*

---

## Overview

Zenon is a permissionless on-chain token launchpad built with Anchor. Anyone can launch a token against a live `Market` — a versioned global config PDA controlled by an admin. Each launch creates a `BondingCurve` PDA that powers a **constant-product AMM** (x × y = k) using both virtual and real reserve tracking. Buyers deposit SOL and receive tokens; sellers return tokens and receive SOL. The curve completes automatically when a configurable `escape_amount` of tokens has been sold, after which the admin can process the accumulated SOL and remaining tokens through dedicated treasury flows. A `tokens_fee_cooldown_timestamp` gates a secondary token withdrawal after completion.

---

## Instructions

| Instruction | Description |
|-------------|-------------|
| `initialize_market` | Admin: create a versioned `Market` PDA with protocol-level parameters (fees, treasuries, mint supply, escape threshold) |
| `update_market` | Admin: update an existing market's configuration |
| `init_token` | Anyone: launch a new token — creates the mint, `BondingCurve` PDA, and curve ATA; mints initial supply; sets virtual/real reserve offsets and cooldown timestamp; renounces mint authority |
| `init_ata` | Anyone (payer): create an associated token account for a given mint and authority |
| `buy_tokens` | Anyone: buy tokens from the bonding curve by depositing SOL; pays a SOL trading fee; triggers curve completion if `escape_amount` is reached |
| `sell_tokens` | Anyone: sell tokens back to the bonding curve in exchange for SOL; pays a SOL trading fee from the seller |
| `process_completed_curve` | Admin: after curve completion — transfer remaining tokens to admin ATA and distribute accumulated SOL to admin and escape fee treasury |
| `withdraw_tokens_fee` | Admin: after curve completion and cooldown — withdraw the token fee allocation from the curve ATA to `tokens_fee_treasury` |

---

## Accounts

| Account | Type | Description |
|---------|------|-------------|
| `Market` | PDA `[b"market", version_le_bytes]` | Global config: authority, initial mint amount, escape amount/fee, trading fee, and treasury destinations |
| `BondingCurve` | PDA `[b"bonding_curve", mint]` | Per-token state: virtual/real SOL and token reserves, token supply, `completed` flag, `tokens_fee_cooldown_timestamp`, linked market |
| `BondingCurveATA` | ATA (authority = `BondingCurve`) | Token account that holds the active bonding curve's token supply |
| Mint | Signer on creation / Account | SPL token mint; mint authority is renounced to `None` after `init_token` |

---

## Roles

Zenon has two effective roles. Understanding exactly what each role is and isn't allowed to do is critical for finding access control bugs.

| Role | How Identified | Legitimate Permissions |
|------|---------------|------------------------|
| **Admin** | `market.authority` — set at `initialize_market`, updatable via `update_market` | Create and update market configuration (`initialize_market`, `update_market`); process completed bonding curves (`process_completed_curve`); withdraw token fees after cooldown (`withdraw_tokens_fee`) |
| **Anyone** | Any signer | Launch new tokens (`init_token`); create ATAs (`init_ata`); buy tokens from the bonding curve (`buy_tokens`); sell tokens back to the bonding curve (`sell_tokens`) |

### What each role explicitly cannot do

- **Anyone** cannot update market parameters — `update_market` requires a signer matching `market.authority`
- **Anyone** cannot call `process_completed_curve` or `withdraw_tokens_fee` — those are gated to `market.authority`
- **Admin** cannot withdraw from an active (non-completed) curve — both post-completion instructions enforce `bonding_curve.completed == true`
- **Admin** cannot withdraw token fees before the cooldown elapses — `withdraw_tokens_fee` checks `clock.unix_timestamp >= bonding_curve.tokens_fee_cooldown_timestamp`
- **No role** can trade on a completed curve — `buy_tokens` and `sell_tokens` both return `BondingCurveCompleted` if the flag is set

---

## Scope

- All code in `src/` is in scope
- Focus on: arithmetic errors in bonding curve math, reserve manipulation (virtual vs real), missing or incorrect account validation, fee calculation bugs, state machine violations (active vs completed), cooldown bypass, lamport transfer safety, PDA seed collisions, slippage check ordering
- Out of scope: test files, Anchor framework bugs, Solana runtime issues

---

## How to Submit

1. **Repost** this week's announcement on X + **comment** under it
2. Submit each finding as a **separate GitHub Issue** using the template below

### Issue Title Format

```
[Week 3] [Severity] Short descriptive title
```

**Examples:**
- `[Week 3] [Critical] Real reserves not updated before completion check allows double-spend`
- `[Week 3] [High] Virtual reserve drift enables price manipulation across buy/sell cycles`
- `[Week 3] [Medium] tokens_fee_cooldown_timestamp set by token creator, not admin`

### Issue Body Template

```markdown
## Finding

**Week**: 3
**Researcher**: [Your GitHub handle + X handle]
**Severity**: [Critical / High / Medium / Low / Informational]
**Category**: [e.g., Arithmetic error, Missing signer check, Logic bug, Access control, State machine bypass, Fee manipulation, etc.]
**Affected function(s)**: [instruction name(s)]

## Description

[Clear explanation of the vulnerability — what's wrong and why it matters]

## Impact

[What can an attacker do? Quantify if possible — e.g., "drain SOL from any bonding curve", "buy tokens at below-curve price"]

## Proof of Concept

> REQUIRED — submissions without a PoC will not be scored.

[Provide a concrete PoC that demonstrates the exploit. This can be:]
- A TypeScript/Rust test that triggers the vulnerability
- A step-by-step transaction sequence with exact account setups
- A code diff showing the exploit path with expected vs actual behavior

[The PoC must be detailed enough for independent verification — no guesswork.]

## Recommended Fix

[How to patch it — include a code snippet if possible]
```

---

*Solana Audit Arena — by Frank Castle*
