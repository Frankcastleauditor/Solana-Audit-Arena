# 🏆 Solana Audit Arena — Leaderboard

**Last updated**: Week 1 — StakeFlow

---

## All-Time Rankings

| Rank | Researcher | Total Points | Criticals | Highs | Mediums | Lows | Weeks Active | Best Week |
|------|-----------|-------------|-----------|-------|---------|------|-------------|-----------|
| 🥇 1 | [@zzzuhaibmohd](https://x.com/zuhaib44) | **15** | 0 | 2 | 0 | 1 | 1 | Week 1 (15 pts) |
| 🥈 2 | [@0xsophon](https://x.com/0xSantii) | **14** | 0 | 1 | 2 | 1 | 1 | Week 1 (14 pts) |
| 🥈 2 | [@novoyd](https://x.com/kyan_novoyd) | **14** | 0 | 1 | 2 | 1 | 1 | Week 1 (14 pts) |
| 🥈 2 | [@4Nescient](https://x.com/4nescient) | **14** | 1 | 0 | 1 | 1 | 1 | Week 1 (14 pts) |
| 5 | [@cdpandora](https://x.com/cd_pandora) | **10** | 1 | 0 | 0 | 0 | 1 | Week 1 (10 pts) |
| 6 | [@alexchenai](https://x.com/AutoPilotAI) | **0** | 0 | 0 | 0 | 0 | 1 | — |
| 6 | [@Zenta00](https://x.com/zenta_sol) | **0** | 0 | 0 | 0 | 0 | 1 | — |
| 6 | [@Stoicov](https://x.com/GeoGen100) | **0** | 0 | 0 | 0 | 0 | 1 | — |
| 6 | [@dexterhere-2k](https://github.com/dexterhere-2k) | **0** | 0 | 0 | 0 | 0 | 1 | — |
| 6 | [@Rayane-Boucheraine](https://x.com/R4Y4N3___) | **0** | 0 | 0 | 0 | 0 | 1 | — |
| 6 | [@BLOCK-PROGRAMR](https://x.com/0x_Scater) | **0** | 0 | 0 | 0 | 0 | 1 | — |
| 6 | [@syed-ghufran-hassan](https://github.com/syed-ghufran-hassan) | **0** | 0 | 0 | 0 | 0 | 1 | — |

*Tiebreak at 14 pts: @0xsophon's first submission (#3, Mar 17 18:36) predates @novoyd's (#11, Mar 18 01:22) which predates @4Nescient's (#12, Mar 18 04:26).*

---

## Weekly Results

### Week 1 — StakeFlow

**Program**: [StakeFlow](https://github.com/Frankcastleauditor/Solana-Audit-Arena/tree/main/programs/stakeflow)
**Category**: Liquid + Locked Staking / Token-2022
**Total Submissions**: 40
**Unique Vulnerabilities Found**: 18 (2C · 4H · 5M · 3L · 4I)
**Researchers**: 12
**Duplicate Rate**: ~32% · **Invalid Rate**: ~25%

| Rank | Researcher | Points | Findings (C/H/M/L/I) | Scoring Issues |
|------|-----------|--------|----------------------|----------------|
| 🥇 1 | [@zzzuhaibmohd](https://x.com/zuhaib44) | **15** | 0/2/0/1/0 | #8, #10, #14 |
| 🥈 2 | [@0xsophon](https://x.com/0xSantii) | **14** | 0/1/2/1/0 | #3, #6, #7, #9 |
| 🥈 2 | [@novoyd](https://x.com/kyan_novoyd) | **14** | 0/1/2/1/0 | #11, #16, #17, #21 |
| 🥈 2 | [@4Nescient](https://x.com/4nescient) | **14** | 1/0/1/1/0 | #12, #18, #19 |
| 5 | [@cdpandora](https://x.com/cd_pandora) | **10** | 1/0/0/0/0 | #20 |
| 6 | [@alexchenai](https://x.com/AutoPilotAI) | **0** | 0/0/0/0/3 | #32, #34, #36 (info only) |
| 6 | [@Zenta00](https://x.com/zenta_sol) | **0** | 0/0/0/0/0 | 1 DQ · 1 invalid |
| 6 | [@Stoicov](https://x.com/GeoGen100) | **0** | 0/0/0/0/0 | 2 invalid |
| 6 | [@dexterhere-2k](https://github.com/dexterhere-2k) | **0** | 0/0/0/0/0 | 1 invalid |
| 6 | [@Rayane-Boucheraine](https://x.com/R4Y4N3___) | **0** | 0/0/0/0/1 | #45 (1 info · 1 dupe) |
| 6 | [@BLOCK-PROGRAMR](https://x.com/0x_Scater) | **0** | 0/0/0/0/0 | 3 dupes |
| 6 | [@syed-ghufran-hassan](https://github.com/syed-ghufran-hassan) | **0** | 0/0/0/0/0 | 1 invalid |

---

## Hall of Fame

### 🏅 Best Finding of the Week

| Week | Researcher | Finding | Why It Stood Out |
|------|-----------|---------|-----------------|
| Week 1 | [@4Nescient](https://x.com/4nescient) | **#12 — Repeatable reward inflation via rounding mismatch in `partial_unstake`** | Independent floor divisions on `reward_debt` create an exploitable gap. Repeating `partial_unstake(1)` + `claim_rewards` inflates rewards by **9900%** with zero new time elapsed. No special privileges required — pure arithmetic attack requiring deep understanding of reward accounting. |

### 🚀 Rising Researcher

| Week | Researcher | Achievement |
|------|-----------|------------|
| Week 1 | [@cdpandora](https://x.com/cd_pandora) | Found the Critical `reward_debt` double-claim bug (#20) in `unstake_locked`. Clean writeup, executable PoC, correct fix proposed. Scored 10 pts as a first-time participant. |

---

*Updated every Monday by [@0xcastle_chain](https://x.com/0xcastle_chain)*
