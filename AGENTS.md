# AGENTS.md — rules for every coding agent in this repo

(Read by Codex, omp, grok and agy. Claude reads it via CLAUDE.md.)

## Project
Micro-voxel diorama engine in Rust: wgpu 30 + winit 0.30 + egui 0.37. One codebase builds the
desktop app (`cargo run --release`) and a WebGPU browser app (`bash scripts/build-web.sh`,
runs on iPad/iPhone). Worlds ("scenes") are generated procedurally from math; game/tool
modes are "experiences".

## Must-read
- `docs/ENGINE.md` — engine API, egui 0.37 differences, screenshot/bench flags, rules.
- Reference world: `src/scenes/sky_islands/` (island.rs, flora.rs, flowers.rs, effects.rs, layout.rs, mod.rs).
- Reference experience: `src/experiences/explore.rs`; trait in `src/engine/experience.rs`.

## Environment (Windows laptop, Intel Iris Xe, 8 cores)
- Rust lives in `%USERPROFILE%\.cargo\bin`. In Bash: `export PATH="$USERPROFILE/.cargo/bin:$PATH"`.
- Other agents compile in parallel: always `CARGO_BUILD_JOBS=2`.
- `cargo build` (dev profile is optimised) for iterating; `--release` for screenshots/bench.

## Rules
- Everything from math: SDFs, height-fields, noise, curves. No hand-placed voxels.
- seed 0 = canonical layout; other seeds vary it deterministically.
- Only edit files your task owns. Registries (`src/scenes/mod.rs`, `src/experiences/mod.rs`): append one line each.
- No changes to existing scenes' look (must stay pixel-identical) unless the task says so.
- Files under 1,000 lines. All user-facing text in English.
- Desktop and web (wasm32) must both compile: no threads/blocking/`std::time::Instant` on wasm.
- No new crates unless the task says so.
- Never commit `.agents-task/`, `.agents-log/`, `target/`, or screenshots.

## Verify
```
cargo build --release                                          # no warnings
cargo test --release
cargo build --release --lib --target wasm32-unknown-unknown
cargo run --release -- <scene> --size 1280x720 --shot 1:0:12:out.png
```
