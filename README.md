# Solana Audit Arena

Weekly Solana smart contract security competition. Run by [Frank Castle](https://x.com/0xcastle_chain).

---

## What it is

A new program drops here every Monday. You get one week to break it.

Each bug goes in as a GitHub Issue with a working PoC. Everything is public. The community picks apart every submission, and I make the final call on validity and severity.

Free. No application. You get judged on what you find.

---

## Why it exists

Plenty of people in Solana can find bugs. Very few have anything public to point at.

Protocols can't tell who is real. Contest platforms help, but they favor researchers who already have time, tooling, and a name.

This is the other door. A realistic codebase every week, a public verdict on your work, and a track record at a URL you can send to anyone.

---

## The targets

Hand written by engineers who audit Solana for a living. Nothing generated. Nothing recycled from old CTF repos.

Every week models something the ecosystem is shipping right now. Agent infrastructure. Delegated spend rails. Launchpads and bonding curves. Token-2022 flows. Staking and vaults. Mixed on-chain and off-chain settlement.

The code reads like a funded team wrote it on a deadline. Full instruction sets, real account structures, real state machines. Bugs sit where they sit in production, not where a tutorial would put them.

I design most weeks. Some go to guest designers from established Solana security teams, announced with the target, so you always know who wrote the code you are attacking.

Difficulty climbs. Early weeks reward pattern recognition. Later ones get quiet.

---

## The cycle

| Day | What happens |
|-----|-------------|
| **Monday** | New target published, announced on X |
| **Monday to Sunday** | Submission window, findings go in as GitHub Issues |
| **Following Monday** | Results scored and posted on X, next target drops |

Window closes Sunday 23:59 UTC. The deadline is hard. Late submissions do not count.

---

## Submitting

One Issue per finding.

**Title:**

```
[Week X] [Severity] Short descriptive title
```

Example: `[Week 3] [Critical] Unauthorized withdrawal via missing signer check in unstake()`

**Body:** use this template. Anything else gets labeled `invalid-format` and is not scored until you fix it, which costs you time you do not have.

```markdown
## Finding

**Week**: [NUMBER]
**Researcher**: [GitHub handle + X handle]
**Severity**: [Critical / High / Medium / Low / Informational]
**Category**: [Missing signer check, arithmetic overflow, PDA seed collision, CPI validation, etc.]
**Affected function**: [instruction or function name]

## Description

What is wrong, and why it matters.

## Impact

What can an attacker do? Be specific. "Drain the vault" beats "could be risky".

## Proof of Concept

REQUIRED. One of:
- A TypeScript or Rust test that triggers the bug
- A step by step transaction sequence with account setups
- A code diff showing the exploit path, expected vs actual

Detailed enough that someone else can verify it without guessing.

## Recommended Fix

How to patch it. Code if you have it.
```

---

## Rules

1. One bug per Issue. Do not bundle.
2. PoC or it is not a finding. "This looks wrong" does not count.
3. Solo entries. Coordinating submissions gets you disqualified.
4. AI and scanners are fine as a starting point. Pasted raw output is not. You explain it, you prove it.
5. No edits to the Issue body after you submit. Add a comment instead.

---

## Judging

Mine. Informed by what the community says in the comments.

I have audited 100+ protocols and found 300+ high and critical severity bugs. Past work with Spearbit (Senior Researcher), Cantina, and Pashov Audit Group.

**Severity**

- **Critical** (10 pts): direct loss of funds, protocol takeover, or permanent freeze. No preconditions beyond a normal transaction.
- **High** (7 pts): real fund loss under specific but realistic conditions, privilege escalation, or a bypass of core access control.
- **Medium** (3 pts): limited loss, denial of service, or state corruption with bounded impact.
- **Low** (1 pt): minor issues and best practice violations. No fund loss.
- **Informational** (0 pts): code quality, docs, theoretical issues with no attack path.

**Scoring**

- First finder takes the points. Duplicates score zero, decided by Issue timestamp. Submit when you are sure. Waiting costs you.
- My severity call is final, after reading the discussion.
- Clear false positives with no analysis cost you 1 point. A reasoned finding that turns out invalid costs you nothing.
- Behavior the target brief documents as intentional is not a finding.

**Labels**

`valid` · `invalid` · `duplicate` · `invalid-format` · `critical` · `high` · `medium` · `low` · `best-find` · `week-N`

---

## Everything is public

On purpose.

Anyone can comment on any Issue. Challenge the severity, question the PoC, suggest a better fix, confirm a finding. Comments do not move the score, but they do shape my read.

If a PoC does not work, say so and explain why. Go after the work, not the person.

Reading other people's submissions will teach you more patterns than any tutorial. Over time the Issues tab becomes a searchable archive of Solana bug classes.

---

## Leaderboard

Standings live in [`LEADERBOARD.md`](./LEADERBOARD.md), updated every Monday.

The weekly results post on X features the top 3, the best finding, and the strongest move from a newer researcher. My audience is Solana researchers, auditors, and protocol teams. That visibility is the point.

---

## Seasons and rewards

The Arena runs in seasons. Each season has its own standings and its own rewards, announced before it starts.

Want to sponsor a season or design a guest week? Find me on X.

---

## FAQ

**New to Solana security. Should I bother?**
Yes. One week of trying to break a real program beats months of reading. Even a week with zero findings pays off if you read what everyone else submitted.

**How much Rust do I need?**
Enough to read it. If you can follow a Solana program and you understand the account model, you are ready.

**Can I use AI?**
As a starting point, yes. You still have to validate it, explain it in your own words, and ship a PoC that runs.

**Won't people copy my finding?**
Timestamps decide. First valid submission scores. Later copies get marked duplicate.

---

## Links

- X: [@0xcastle_chain](https://x.com/0xcastle_chain)
- GitHub: [Frankcastleauditor](https://github.com/Frankcastleauditor)
