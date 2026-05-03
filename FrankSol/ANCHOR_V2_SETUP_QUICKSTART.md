# Anchor v2 Setup Quickstart (macOS + Revert Guide)

This is the short operational guide for contributors working on this repo after the Anchor v2 migration.

For full migration details, see:

- `ANCHOR_V2_MIGRATION_NOTES.md`

## 1) Install Anchor v2 (`anchor-next`)

Anchor v2 is currently unreleased (alpha) and installed from git.

### Standard install

```bash
cargo install --git https://github.com/solana-foundation/anchor.git --branch anchor-next anchor-cli --force
```

### macOS fallback (if linker/bitcode errors occur)

```bash
CARGO_PROFILE_RELEASE_LTO=off cargo install --git https://github.com/solana-foundation/anchor.git --branch anchor-next anchor-cli --force
```

Verify:

```bash
anchor --version
```

## 2) Build and test this repo

From repo root (`stake_v2/`):

```bash
cargo check --manifest-path Cargo.toml
cargo test --manifest-path Cargo.toml
```

Optional:

```bash
anchor build
anchor test
```

## 3) If you are currently on Anchor v1

Installing v2 with `cargo install ... --force` will replace your existing `anchor` binary on `PATH`.

Before switching, record current state:

```bash
which anchor
anchor --version
which avm || true
avm --version || true
```

## 4) Revert back to stable Anchor (recommended: AVM)

If you use AVM:

```bash
avm list
avm install <stable-version>
avm use <stable-version>
anchor --version
```

Example:

```bash
avm install 0.30.1
avm use 0.30.1
```

If AVM is not installed, install AVM first and switch via AVM to avoid repeated global overwrites.

## 5) Reproducibility recommendation

`anchor-next` moves quickly. For production/review stability:

- pin `anchor-lang-v2` and `anchor-spl-v2` to a specific git `rev`,
- include tool versions in PR description (`anchor --version`, rust/cargo versions).
