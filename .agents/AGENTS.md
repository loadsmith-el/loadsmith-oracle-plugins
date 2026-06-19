# AI Agent Guidelines — loadsmith-oracle-plugins

> Source of truth for this repository, for any AI agent (Claude, Codex, Gemini,
> …). The root `CLAUDE.md` is only a pointer to this folder.

## Golden Rule (the `.agents/` folder prevails)

The `.agents/` folder is the source of truth. If existing code conflicts with
what is documented here, the documented standard prevails — surface the conflict
to the user before diverging.

## Authoring rule — how to extend this (all agents MUST follow)

This `.agents/` folder is the **single source of truth for every AI agent**
(Claude, Codex, Gemini, …) — not a reference copy. When you add or change agent
guidance, you MUST keep that truth here:

- **A new directive / convention / rule** → add it to **this file**
  (`.agents/AGENTS.md`). Do **not** put it in `CLAUDE.md` or any other per-agent
  file — those are pointers, not content.
- **A new skill / command** → write the real, agent-agnostic logic in
  **`.agents/skills/<name>.md`**. Then wire each agent's native entry point as a
  **thin stub** that only redirects here:
  - Claude: `.claude/commands/<name>.md` — keep its frontmatter (`description`,
    `argument-hint`, `allowed-tools`) so the slash command registers, then a body
    that says "read and follow `.agents/skills/<name>.md`".
  - Other agents: their own command/skill mechanism, with the same redirect.
- **Never** duplicate real instructions or skill logic into `CLAUDE.md` or into a
  stub. If a per-agent file ever starts holding real content, move it here and
  leave a pointer behind.
- Committed files stay in **English** (repo rule), even when chatting in another
  language.

Operating instructions for this repo. For *what* it is and *why*, read
[README.md](../README.md) and the [docs](../docs).

## Conventions

- **English only.** All committed artifacts — code, comments, commit messages,
  identifiers, docs — in English, even when the user writes in Portuguese.
- **This is out of canon on purpose.** It links the native Oracle client
  (ODPI-C / Instant Client). Do NOT try to make it pure-Rust or add it to the
  canonical index — that's the *future* path (via the `oracle-rs` fork), not this
  repo. Keep the canonical/pure-Rust posture for the OTHER plugins.
- **Mirror the canonical connector shape:** one crate, shared `conn`/`types`,
  two separate binaries (`src/bin/source.rs`, `src/bin/destination.rs`). The
  driver-agnostic SQL (projected `TO_CHAR` reads, `TO_*` binds, `MERGE`) is
  carried over verbatim from the canonical Oracle connector — keep it that way so
  it stays portable back to pure-Rust later.
- **Keep docs in sync.** Changing behavior/commands/shipping → check README,
  docs/, ROADMAP, and the image/lab READMEs.

## The async↔sync bridge

The `oracle` crate is synchronous; the SDK traits are async. Every DB call runs
inside `tokio::task::block_in_place` (the plugin bins use the multi-thread
runtime). `oracle::Connection` is `Send + Sync`, so it lives in the plugin struct
across calls. Don't reach for `spawn_blocking` (it would force moving the
non-`'static` connection); `block_in_place` is the pattern here.

## Build / test

```bash
cargo build                          # compiles bundled ODPI-C C (needs `cc`)
cargo test -p loadsmith-oracle       # unit tests only (no DB, no Instant Client)
```

Building needs a C compiler but NOT the Instant Client (ODPI-C dlopens
`libclntsh` at runtime). Live behavior needs a real Oracle + the client — see
[lab/README.md](../lab/README.md).

## Releasing

Tag `oracle-v<version>` (matching `connector/Cargo.toml`). The
[release workflow](../.github/workflows/release.yml) builds the binaries
(amd64+arm64), publishes a GitHub Release + updates `index.json`, and builds +
pushes the `loadsmith-oracle-odpi` image. Push tags one at a time.
