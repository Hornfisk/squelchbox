# Contributing to SquelchBox

Thanks for your interest! Bug reports, feature ideas, and pull requests are welcome.

## Build from source

```bash
git clone https://github.com/Hornfisk/squelchbox.git
cd squelchbox
cargo build --release
cargo run --release --bin squelchbox-standalone
```

Plugin bundles are produced via:

```bash
cargo xtask bundle squelchbox --release
```

Artefacts land in `target/bundled/` (VST3 + CLAP) and `target/release/squelchbox-standalone`.

## Tests

```bash
cargo test --release
```

The suite covers DSP (envelopes, filter, FX chain, limiter), the sequencer, and parameter plumbing. Please keep it green before opening a PR.

## Code style

- `cargo fmt` before every commit; `cargo clippy --all-targets` should be clean.
- Keep audio-thread code allocation-free and lock-free. The `KbdQueue` + `pattern_rev()` pattern is how the GUI talks to audio — mirror it for new cross-thread state.
- DSP units live in `src/dsp/`, UI panels in `src/ui/panels/`, layout constants in `src/ui/palette.rs`.
- See `CLAUDE.md` for the high-level architecture tour.

## Pull requests

- Branch from `main`, open the PR against `main`.
- One feature/fix per PR. Reference an issue if one exists.
- Include a short note in `CHANGELOG.md` under an `## [Unreleased]` section if your change is user-visible.
