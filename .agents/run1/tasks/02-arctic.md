---
id: 02-arctic
type: code
agent: omp:deepseek-v4-pro
acceptance: [W1, W2, W3, W4, W5, W6, W7, G1, G2]
---

# Task 02: Aurora Village world

You are one of several agents working in parallel in separate git worktrees (your current
directory is your worktree). Work fully autonomously; never ask questions.

## Context
- Read `AGENTS.md` and `docs/ENGINE.md` first. Then study `src/scenes/sky_islands/`
  (island.rs, flora.rs, effects.rs, layout.rs, mod.rs) and `src/engine/{scene.rs,sdf.rs,voxel.rs,math.rs}`.
  Copy its structure and idioms.
- Shell setup: `export PATH="$USERPROFILE/.cargo/bin:$PATH" CARGO_BUILD_JOBS=2`.

## You own
- `src/scenes/arctic/**` (new; split into several files, each < 1,000 lines)
- Append exactly one module line and one `SceneEntry { id: "arctic", title: "Aurora Village", build: arctic::build }` line to `src/scenes/mod.rs`.

## Build this: "Arctic Aurora Village" (id `arctic`)
- Snowy rolling terrain (noise height-field) around a frozen lake (translucent pale-blue ice).
- Icebergs / ice ridges on the lake edge: noise-displaced SDF blobs.
- Igloos: SDF hemisphere shells with entrance tunnels and block-seam grooves.
- Cozy timber cabins with snowy roofs and warm lit windows (emissive voxels / lamp material).
- Pine forest: stacked cones with snow caps, placed via Poisson disc outside the village.
- **Aurora**: several long translucent emissive ribbon/curtain meshes high in the sky, animated as dynamic instances swaying via sine/noise in `update_dynamic_instances`; strongest at night.
- Falling snow particles.
- Night is the showcase (stars on). seed ≠ 0: vary lake shape, village layout, forest. Summer season: less snow, meltwater/greener patches.
- 6–12 landmarks (e.g. "Frozen Lake", "Igloo Camp", "Lodge", "Aurora Ridge"), 4–9 camera presets, a tour, three time modes.

## Acceptance criteria
- [ ] W1–W7 and G1–G2 from `.agents-task/spec.md` (read it).

## Verify (all must pass before committing)
```
cargo build --release            # zero warnings
cargo test --release
cargo build --release --lib --target wasm32-unknown-unknown
cargo run --release -- arctic --size 1280x720 --shot N:0:12:.agents-log/shots/arctic-N-day.png    # every preset N
cargo run --release -- arctic --size 1280x720 --shot N:2:12:.agents-log/shots/arctic-N-night.png
cargo run --release -- arctic --bench --size 1920x1080
```
If you can view images, open and LOOK at the screenshots and iterate until it looks great.

## Finish
`git add -A && git commit -m "feat(arctic): Aurora Village world"`. Report: what you built (math
used), triangle count, bench numbers, generation time, known issues.
