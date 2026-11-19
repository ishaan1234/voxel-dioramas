# Engine guide (for contributors)

Rust + wgpu 30 + winit 0.30 + egui 0.37. One codebase builds the desktop app
(`cargo run --release`) and the WebGPU browser app (`bash scripts/build-web.sh`).

## Layout

```
src/engine/        renderer + framework (shared; change only what your task needs)
  app.rs           event loop, input routing, scene loading, experience switching
  gpu.rs           renderer (GpuState): scene buffers, shadows, weather, props, egui paint
  scene.rs         Scene trait, SceneData, SceneBuilder, WorldParams, Season, Landmark
  experience.rs    Experience trait, Ctx, Requests
  atmosphere.rs    DayClock (presets / continuous / live), Weather + precipitation
  props.rs         runtime meshes + per-frame instances for experiences
  storage.rs       persistent key/value (files on desktop, localStorage on web)
  ui.rs            egui layer + global menu bar
  sdf.rs, voxel.rs, math.rs   the math toolkit (SDFs, voxeliser, noise, curves)
src/scenes/<id>/   one directory per world; registered in src/scenes/mod.rs
src/experiences/   one file or directory per mode; registered in src/experiences/mod.rs
```

## Worlds (scenes)

`pub fn build(params: &WorldParams) -> Box<dyn Scene>`

- **Everything from math.** Shapes are SDFs / height-fields / parametric curves
  rasterised with `MicroVoxelVolume::{fill_sdf, carve_sdf, fill_columns, sweep}`;
  placement uses noise, golden spirals, Poisson discs. No hand-placed voxels.
- `params.seed == 0` must be the canonical hand-tuned layout. Other seeds vary it.
  Derive all randomness from `Rng(seed-derived value)`; keep generation deterministic.
- `params.season` should change palettes / foliage / snow cover where it makes sense.
- Implement `surface_y` (ground height, used for picking / gameplay), `as_any`, and
  fill `SceneData::landmarks` (named points of interest).
- Provide 3–9 camera presets, a tour, three `time_modes` (day, golden hour, night).
- Build heavy work with `par_map` (threads on desktop, sequential on web).
- Use `SceneBuilder`: `add_mesh` (static), `add_dynamic` (animated; emit instances in
  the same order in `update_dynamic_instances`), `add_detail` (many small LOD props).
- Keep the triangle budget sane: the target device is an Intel Iris Xe laptop and an
  iPad. Check `--bench` (aim for ≤ ~20 ms average at 1080p on this laptop).

## Experiences (modes)

Implement `engine::experience::Experience` (all hooks optional except `id`):
`enter`, `exit`, `scene_loaded`, `update`, `ui(&egui::Context, &mut Ctx)`,
`window_event` (return true to consume), `free_camera`, `immersive`.

`Ctx` gives you: `scene` (downcast via `as_any`), `scene_id`, `params`, `cam`
(`set_view(eye, target)` to drive it; `free_camera() == false` disables default
controls), `atmo` (clock / weather), `props`, `storage`, `requests`
(`load_scene`, `switch_experience`, `capture`, `toast`), `time`, `dt`, `screen`,
`screen_ray`, `pick_ground`, `world_to_screen`, `in_view`.

Props: `let id = ctx.props.add_mesh(&mesh, Pass::Opaque)` once (props are cleared on
every scene load, so (re)create them in `scene_loaded`), then
`ctx.props.push(id, ModelInstance::from_trs(..))` every frame.

## egui 0.37 notes (differs from older egui)

- Panels attach to the root Ui: `ctx.root_ui(|ui| egui::Panel::top("id").show(ui, |ui| ...))`
  (also `Panel::bottom/left/right`). `egui::Window::new(..).show(ctx, ..)` and
  `egui::Area::new(egui::Id::unique("name")).show(ctx, ..)` take the Context.
- `Id::new` is deprecated: use `Id::unique("...")`.
- Input queries: `ctx.egui_wants_pointer_input()`, `ctx.egui_wants_keyboard_input()`.
- Fonts: default fonts + monochrome emoji are enabled; prefer plain words over
  unusual symbols (some glyphs such as ☰ / ✕ are missing).
- Make touch targets ≥ 44 px; the iPad has no keyboard or right mouse button.

## Verify before you finish

```
cargo build --release                                     # no warnings
cargo test --release
cargo build --release --lib --target wasm32-unknown-unknown   # web build compiles
cargo run --release -- <scene> --size 1280x720 --shot 1:0:12:out.png   # look at it
cargo run --release -- <scene> --bench --size 1920x1080
```

Screenshot flags: `--shot preset:time:seconds:path` (time 0 day / 1 golden / 2 night),
`--view ex,ey,ez,tx,ty,tz`, `--weather rain|snow|storm|mist|cloudy`, `--hour 0-24`,
`--seed N`, `--season spring|summer|autumn|winter`, `--mode <experience id>`.

## Rules

- No quality regressions to existing scenes (pixel-identical unless the change is
  intended). Files stay under 1,000 lines. All user-facing text in English.
- Desktop and web must both work: no threads, blocking or `std::time::Instant`
  on wasm (use `web_time`, `par_map`, `cfg(target_arch = "wasm32")`).
