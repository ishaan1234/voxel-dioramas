---
id: 03-photo
type: code
agent: grok
acceptance: [P1, P2, P3, P4, P5, G1, G2]
---

# Task 03: Photo Hunt game mode

You are one of several agents working in parallel in separate git worktrees (your current
directory is your worktree). Work fully autonomously; never ask questions.

## Context
- Read `AGENTS.md` and `docs/ENGINE.md` first (Experience trait, Ctx, egui 0.37 notes).
  Study `src/engine/{experience.rs,app.rs,camera.rs,scene.rs,storage.rs,atmosphere.rs,ui.rs}`,
  `src/experiences/explore.rs`, and how `src/scenes/sky_islands/` fills `SceneData::landmarks`.
- Shell setup: `export PATH="$USERPROFILE/.cargo/bin:$PATH" CARGO_BUILD_JOBS=2`.

## You own
- `src/experiences/photo/**` (or `photo.rs`)
- Append one module line and one `ExperienceEntry { id: "photo", title: "Photo Hunt", blurb: "...", make: photo::make }` line to `src/experiences/mod.rs`.
- Do not edit engine files. If a tiny hook is truly unavoidable, keep it < 15 lines and comment it.

## Build this: Photo Hunt (id `photo`)
- On `scene_loaded`, generate 6–10 photo targets deterministically from the scene's landmarks plus
  conditions, e.g. "<landmark> at night", "<landmark> during golden hour", "<landmark> in the rain",
  "<landmark> close up" (distance threshold), "<landmark> from far away". Conditions read `ctx.atmo`
  (clock phase / weather) and the camera distance.
- Camera = default free camera (`free_camera() -> true`).
- Shutter: big egui button (≥ 60 px, bottom centre, touch friendly) and keyboard F. On press:
  evaluate every target — visible (`ctx.in_view` / `ctx.world_to_screen`), centredness (distance from
  screen centre), size on screen, conditions met → score 0–3 stars. White flash (fading full-screen
  egui overlay) + a result card listing what was captured.
- Album: panel (toggle button + keyboard G) showing found/total, each target's name, stars, best score,
  and a hint for unfound ones ("Try visiting at night", "Get closer"). Persist per scene in `ctx.storage`
  (key like `photo.album.<scene_id>`), serialised simply (no new crates).
- Pure logic (target generation, scoring, album (de)serialisation) in a separate module with unit tests.

## Acceptance criteria
- [ ] P1–P5 and G1–G2 from `.agents-task/spec.md` (read it).

## Verify (all must pass before committing)
```
cargo build --release            # zero warnings
cargo test --release
cargo build --release --lib --target wasm32-unknown-unknown
cargo run --release -- sky-islands --mode photo --size 1280x720 --shot 1:0:12:.agents-log/shots/photo-ui.png
```
(Note: `--shot` may render without running experience UI; if the HUD does not show, say so in the
report rather than changing engine code.)

## Finish
`git add -A && git commit -m "feat(photo): Photo Hunt mode"`. Report: design, scoring formula,
controls, test results, known issues.
