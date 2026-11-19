---
id: 01-desert
type: code
agent: agy
acceptance: [W1, W2, W3, W4, W5, W6, W7, G1, G2]
---

# Task 01: Desert Canyon world

You are one of several agents working in parallel in separate git worktrees (your current
directory is your worktree). Work fully autonomously; never ask questions.

## Context
- Read `AGENTS.md` and `docs/ENGINE.md` first. Then study `src/scenes/sky_islands/`
  (island.rs, flora.rs, observatory.rs, bridges.rs, effects.rs, layout.rs, mod.rs) and
  `src/engine/{scene.rs,sdf.rs,voxel.rs,math.rs}`. Copy its structure and idioms.
- Shell setup: `export PATH="$USERPROFILE/.cargo/bin:$PATH" CARGO_BUILD_JOBS=2`.

## You own
- `src/scenes/desert/**` (new; split into several files, each < 1,000 lines)
- Append exactly one module line and one `SceneEntry { id: "desert", title: "Desert Canyon", build: desert::build }` line to `src/scenes/mod.rs`.

## Build this: "Desert Canyon & Ruins" (id `desert`)
- Terrain: ridged-noise mesas and canyon walls with layered sandstone strata colours (colour by height bands + noise), dunes (sine + noise height-field) on the open floor.
- A winding dry riverbed (parametric curve carved into the floor) ending in an oasis pool with palms (curved trunk sweep + fan fronds).
- An eroded natural arch: SDF torus/capsule minus noise, attached to a mesa.
- Ruined sandstone temple / colonnade: columns with noise-chipped broken tops placed by polar repetition, fallen column drums, a stepped platform.
- Cacti (capsule arms), scattered rocks via Poisson-disc placement.
- Effects: drifting sand particles by day; night: starry sky and warm torch point lights at the ruins.
- seed ≠ 0: vary mesa layout, river path, ruin position/orientation, dune phase. Winter season: frost on tops + cooler palette; other seasons may stay the same.
- 6–12 landmarks (e.g. "Great Arch", "Sun Temple", "Oasis", "Canyon Overlook"), 4–9 camera presets, a tour, three time modes.

## Acceptance criteria
- [ ] W1–W7 and G1–G2 from `.agents-task/spec.md` (read it).

## Verify (all must pass before committing)
```
cargo build --release            # zero warnings
cargo test --release
cargo build --release --lib --target wasm32-unknown-unknown
cargo run --release -- desert --size 1280x720 --shot N:0:12:.agents-log/shots/desert-N-day.png    # every preset N
cargo run --release -- desert --size 1280x720 --shot N:2:12:.agents-log/shots/desert-N-night.png
cargo run --release -- desert --bench --size 1920x1080
```
Open and LOOK at the screenshots (you can read images). Iterate until it looks great.

## Finish
`git add -A && git commit -m "feat(desert): Desert Canyon world"`. Report: what you built (math
used), triangle count, bench numbers, generation time, known issues.
