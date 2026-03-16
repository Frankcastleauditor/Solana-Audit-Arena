# ⚔️ Solana Audit Arena

**A weekly Solana smart contract security competition — by [Frank Castle](https://x.com/0xcastle_chain)**

---

## What Is This?

The Solana Audit Arena is a free, open, weekly competition where security researchers audit AI-generated Solana programs to find as many vulnerabilities as they can.

Every Monday, a new Anchor program is published here. All programs are built using the [Safe Solana Builder](https://github.com/Frankcastleauditor/safe-solana-builder) — a security-focused Solana program generator that enforces safe patterns and audit-derived rules. You have **one week** to find bugs, write a PoC, and submit your findings as a GitHub Issue. The community reviews and discusses every submission publicly. Frank Castle — an expert Solana security researcher with 100+ protocol audits — makes the final call on validity and severity.

**This exists because junior security researchers deserve a proving ground.** There's no clear path for newcomers to sharpen their skills against realistic Solana codebases, compete on merit, and get noticed. This arena is that path.

---

## Why Participate?

- **Build your audit track record** with verified findings on a public leaderboard
- **Learn from real vulnerability patterns** across DeFi, staking, governance, stablecoins, and more
- **Get community feedback** — every submission is publicly reviewed and discussed
- **Get noticed** — top performers get highlighted on Frank Castle's X (4k+ security-focused followers)
- **Top prize**: The leading researcher gets **invited to join a private audit engagement with Frank Castle** — real paid work, real experience, real credential.

---

## How It Works

### Weekly Cycle

| Day | What Happens |
|-----|-------------|
| **Monday** | New program published → Announcement post on X with link to this repo |
| **Monday–Sunday** | Open submission window → Submit findings as GitHub Issues |
| **Following Monday** | Frank validates, scores, and posts results on X → New program announced |

### Timeline

- **Submission window**: Monday 00:00 UTC → Sunday 23:59 UTC (7 full days)
- **Community review**: Ongoing — anyone can comment on any submission during and after the window
- **Final results**: Published the following Monday alongside the new program announcement
- **Late submissions**: Not accepted. The deadline is hard.


---

## Submission Format

Submit each finding as a **separate GitHub Issue** in this repository.

### Issue Title Format

```
[Week X] [Severity] Short descriptive title
```

Example: `[Week 3] [Critical] Unauthorized withdrawal via missing signer check in unstake()`

### Issue Body Template

Use this template exactly — issues that don't follow the format will be tagged `invalid-format` and won't be scored until corrected (eating into your submission window).

```markdown
## Finding

**Week**: [NUMBER]
**Researcher**: [Your GitHub handle + X handle]
**Severity**: [Critical / High / Medium / Low / Informational]
**Category**: [e.g., Missing signer check, Arithmetic overflow, PDA seed collision, CPI validation, etc.]
**Affected function**: [instruction name or function]

## Description

[Clear explanation of the vulnerability — what's wrong and why it matters]

## Impact

[What can an attacker do? Quantify if possible — e.g., "drain all vault funds", "bypass admin check"]

## Proof of Concept

- REQUIRED

[Provide a concrete PoC that demonstrates the exploit. This can be:]
- A TypeScript/Rust test that triggers the vulnerability
- A step-by-step transaction sequence with account setups
- A code diff showing the exact exploit path with expected vs actual behavior

[The PoC must be detailed enough that someone can independently verify the vulnerability without guesswork.]

## Recommended Fix

[How to patch it — include code if possible]
```

### Issue Labels

Issues will be labeled by Frank Castle during judging:

| Label | Meaning |
|-------|---------|
| `valid` | Confirmed vulnerability, scored |
| `invalid` | Not a real vulnerability |
| `duplicate` | Same finding submitted earlier by another researcher |
| `invalid-format` | Doesn't follow the submission template |
| `critical` / `high` / `medium` / `low` | Final severity assigned by judge |
| `best-find` | Best finding of the week |
| `week-N` | Which week the finding belongs to |

---

## Community Review

**Every submission is public.** This is intentional.

- **Anyone** can comment on any GitHub Issue — challenge the severity, question the PoC, suggest a better fix, or confirm the finding
- Community discussion is encouraged and helps everyone learn
- Comments do **not** affect scoring — only Frank Castle's final judgment determines points
- Be constructive. Tearing down someone's submission without explanation is not helpful
- If you find that someone's PoC doesn't actually work, explain why — that's valuable feedback

### Why Public Submissions?

1. **Transparency** — everyone can see how findings are judged
2. **Learning** — reading other researchers' submissions teaches you new patterns
3. **Accountability** — PoCs are verified by the community, not just one person
4. **Archive** — the Issues tab becomes a searchable database of Solana vulnerability patterns

---

## Scoring

| Severity | Points |
|----------|--------|
| Critical | 10 |
| High | 7 |
| Medium | 3 |
| Low | 1 |
| Informational | 0 (acknowledged but no points) |

### Severity Definitions

- **Critical**: Direct loss of funds, total protocol takeover, or permanent freeze of all assets. No preconditions beyond a normal transaction.
- **High**: Significant loss of funds under specific but realistic conditions, privilege escalation, or bypass of core access control.
- **Medium**: Limited fund loss, denial of service, or state corruption that requires specific conditions or has a bounded impact.
- **Low**: Minor issues, best-practice violations, or gas optimizations that do not lead to fund loss.
- **Informational**: Code quality suggestions, documentation gaps, or theoretical issues with no practical attack path.

### Scoring Rules

- **First finder gets full points.** If multiple researchers report the same vulnerability, only the first valid submission (by GitHub Issue timestamp) earns points. Duplicates are labeled `duplicate` and score 0.
- **PoC is mandatory.** Submissions without a working Proof of Concept will be labeled `invalid-format` and won't be scored. No exceptions — if you can't prove it, it's not a finding.
- **Severity is final.** Frank Castle assigns the final severity after considering community discussion. The classification stands for scoring purposes.
- **Invalid findings**: Submitting findings that are clearly not vulnerabilities (false positives with no analysis) may result in a -1 point penalty per invalid finding to discourage spam. Use judgment — when in doubt, explain your reasoning and it won't count against you.

---

## Submission Rules

1. **One Issue per finding.** Don't bundle multiple vulnerabilities into one Issue. Each vulnerability gets its own Issue with its own PoC.
2. **Individual only.** No team submissions. You can discuss general Solana security concepts publicly, but coordinating submissions is grounds for disqualification.
3. **Original work only.** Running the program through an automated scanner and pasting raw output is not accepted. You must demonstrate understanding in your description and provide a real PoC.
4. **PoC required.** Every submission must include a Proof of Concept that independently verifies the vulnerability. "This looks wrong" is not a finding. "Here's exactly how to exploit it" is.
5. **No editing after submission.** Once you submit an Issue, do not edit the body. If you need to add context, add a comment. Edits to the original Issue body after submission may result in disqualification for that finding.

---

## Leaderboard

The **all-time leaderboard** is maintained in [`LEADERBOARD.md`](./LEADERBOARD.md) in this repository and updated every Monday with the week's results.


### Highlights

Each week, the results post on X will feature:
- **Top 3 researchers** of the week
- **Best finding** of the week (most creative or impactful)
- **Rising researcher** — biggest improvement from a newer participant

---


## Judging

All submissions receive their **final judgment** from **Frank Castle** ([@0xcastle_chain](https://x.com/0xcastle_chain)), informed by community discussion.

Frank has audited 100+ protocols and 50+ Solana programs, identifying 300+ high and critical severity vulnerabilities. Previous engagements include Cantina, Spearbit (Senior Researcher), and Pashov Audit Group.

---


## FAQ

**Q: I'm a complete beginner. Can I participate?**
A: Absolutely. That's who this is for. You'll learn more from one week of trying to break a real program than from months of tutorials. Even if you find 0 bugs your first week, you'll learn from reading other people's submissions.

**Q: Do I need to be a Rust expert?**
A: You need to be able to read Rust and understand Solana's account model. If you can follow an Anchor program's logic, you're ready.

**Q: Is there a cost to participate?**
A: No. Free. Always.

**Q: Can I use AI tools to help me audit?**
A: Yes — but you must understand and validate every finding you submit. Raw scanner output without analysis will be rejected. If you use AI as a starting point and then verify and explain the finding yourself, that's fair game. Your PoC still needs to work.

**Q: Will programs get harder over time?**
A: Yes. Early programs will have more obvious bugs. As the community levels up, so will the complexity.

**Q: How do I get the "join a private audit" prize?**
A: Be the leading researcher on the all-time leaderboard at evaluation points (announced in advance). This isn't just about points — consistency, finding quality, and demonstrated growth all factor in.

**Q: Won't public submissions let people copy each other?**
A: Timestamps matter. First valid submission gets the points. If someone submits after you with the same finding, they get labeled `duplicate`. This actually rewards speed and confidence — submit when you're sure, don't wait.

**Q: Can I comment on other people's submissions?**
A: Yes — that's the point. Community review makes everyone better. Challenge PoCs, suggest better fixes, confirm findings. Just be constructive.

---

## Links

- **X**: [@0xcastle_chain](https://x.com/0xcastle_chain)
- **GitHub**: [Frankcastleauditor](https://github.com/Frankcastleauditor)
- **Safe Solana Builder**: [github.com/Frankcastleauditor/safe-solana-builder](https://github.com/Frankcastleauditor/safe-solana-builder)

---

*Built by Frank Castle. Securing Solana, one researcher at a time.*
