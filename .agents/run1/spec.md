# Spec: Experience pack, part 1 (pipeline test run)

Run id: run1   Base: agents/run1 (from b519545)   Date: 2026-11-19

## Goal
Continue the "turn the diorama into experiences" roadmap with two new math-generated worlds,
one new game mode, and a social-preview image for the web build. Also serves as the first
test of the multi-agent pipeline (Claude plans → agy/grok/omp build → Codex reviews).

## Decisions
- Roadmap from the previous session (items 1–7, minus 10). This run takes a slice:
  Desert Canyon (item 1a), Aurora Village (item 1b), Photo Hunt (item 4b), plus a web og-image.
- Remaining items (reef, neon/clockwork/hub, seasons/seeds/grow, courier, builder/explorer,
  island tender, wallpaper/music/postcards) are for later runs.

## Acceptance criteria
### Worlds (apply to each of desert and arctic)
- **W1** New directory `src/scenes/<id>/`, `pub fn build(params: &WorldParams) -> Box<dyn Scene>`, registered with one appended line in `src/scenes/mod.rs`.
- **W2** All geometry from math (SDFs / height-fields / noise / curves via MicroVoxelVolume & SceneBuilder); no hand-placed voxels.
- **W3** seed 0 = canonical layout; other seeds deterministically produce a different but valid layout; `params.season` changes palette/snow where meaningful.
- **W4** Implements `surface_y`, `as_any`, 6–12 named landmarks, 4–9 camera presets, a tour, three time modes (day, golden hour, night).
- **W5** Consistency test using `crate::engine::scene::assert_scene_consistent` (seed 0 and at least two other seeds).
- **W6** Performance: ≤ ~3M triangles, generation ≤ ~3 s release, `--bench` reported.
- **W7** Looks good: screenshots of every preset at day + night show readable composition, no holes, no floating/clipping geometry, no z-fighting.

### Photo Hunt
- **P1** Experience id `photo`, title "Photo Hunt", in `src/experiences/photo/` (or photo.rs), registered with one appended line.
- **P2** Targets are generated from the current scene's landmarks plus conditions (time of day via `ctx.atmo`, weather, distance, near screen centre via `ctx.world_to_screen` / `ctx.in_view`); works on any scene with landmarks.
- **P3** Big shutter button (≥ 60 px, touch friendly) and keyboard F take a photo: framing score (centredness, size on screen, conditions), flash + result card.
- **P4** Album persisted per scene via `ctx.storage`: found/total, names, stars, best score; hints for unfound targets.
- **P5** Unit tests for scoring and album persistence round-trip.

### Social preview image
- **I1** `web/og-image.png`, 1200×630, showing a pastel voxel/diorama sakura-temple floating island, title area left clear; `web/index.html` gets `og:image`, `og:title`, `og:description`, `twitter:card` meta tags.

### Global
- **G1** `cargo build --release` with no warnings; `cargo test --release` passes; `cargo build --release --lib --target wasm32-unknown-unknown` compiles.
- **G2** Existing worlds unchanged; files < 1,000 lines; English text only.

## Task map
| Task | Type | Agent | Owns | ACs |
|---|---|---|---|---|
| 01-desert | code | agy | src/scenes/desert/** + 1 registry line | W1–W7, G1–G2 |
| 02-arctic | code | omp:deepseek-v4-pro | src/scenes/arctic/** + 1 registry line | W1–W7, G1–G2 |
| 03-photo | code | grok | src/experiences/photo/** + 1 registry line | P1–P5, G1–G2 |
| 04-ogimage | image | agy (gen-image) | web/og-image.png, web/index.html meta | I1 |
