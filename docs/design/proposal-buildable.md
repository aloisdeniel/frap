# frap architecture proposal: buildability first

Status: proposal (2026-11-16). Target: Blender 5.2 LTS Python API. Toolchain: Rust 1.97 stable.
Inputs: `docs/research/blender-ops-object.md`, `blender-ops-mesh-uv.md`, `blender-rna-datamodel.md`, `gltf-and-blender-io.md`, `rust-ecosystem.md`. Section numbers such as "[ops-mesh §2.4]" point into those documents.

**Angle of this proposal.** The architecture is chosen so that many agents can build frap in parallel with few merge conflicts:

- Every crate has a narrow, frozen interface.
- Operators, modifiers, RNA functions and glTF extensions each register from **their own file**. There is no central list to edit.
- Everything Blender already defines (operator names, property names, defaults, enum items, RNA struct layouts, node sockets, modifier settings) is **generated from dumps of the real Blender 5.2.4**, never typed by hand.
- Each milestone ships a working `frap` binary with a test suite.
- The default build needs only `cargo` and a C/C++ compiler. It never needs cmake, libclang or network access in `build.rs`.

---

## 0. Executive summary

| Concern | Decision |
|---|---|
| Workspace | About 30 small crates in 11 dependency layers (§3). Wide, shallow layers mean low layers freeze early and high layers parallelise. |
| Registration | `linkme` distributed slices plus `automod` directory modules, so adding an operator means adding one file (§1, §10). |
| Blender metadata | `tools/blender/*.py` dump RNA, operators, nodes, modifiers, startup scene and glTF exporter props into `schema/blender-5.2/*.json`. `cargo xtask gen` turns them into checked-in Rust: typed operator property structs, `StructDef` tables, plain-data structs with fresh-datablock defaults, and node socket tables (§4). |
| Scene model | `Main` holds per-type `IdMap<T>` of `Arc<T>`. Writes go through `Arc::make_mut`. That gives O(#IDs) snapshots for rollback, automatic change tracking, and generation counters for evaluation caching (§5, §12). |
| Mesh | `frap-mesh::Mesh` is Blender's struct-of-arrays attribute mesh. `frap-bmesh::BMesh` is the edit structure: disk and radial cycles in slot pools with a **LIFO free list identical to `BLI_mempool`**, so element order can match Blender (§6). |
| Evaluation | `frap-eval` is a lazy, generation-keyed "depsgraph-lite": shape keys, modifier stack, armature deformation, world matrices, bounds. Modifiers are plugins with one file each (§7). |
| RNA | A generic property engine in `frap-rna`: `PropDef`/`StructDef`, one coercion routine that copies Blender's rules and error wording, `RnaPtr` paths, local `Rna` traits on data structs, plus globally registered function and setter hooks for anything that touches more than one datablock (§9). |
| Operators | Generated `XxxProps` structs with `Prop<T>{value, is_set}`. The implementer writes one `#[operator]` function and a poll. Macro operators, dynamic enums and "Blender has it but frap doesn't yet" are handled by the framework (§10). |
| Front-ends | One hand-written lexer and two parsers (bpy-line, short form) lower to one IR. `Session::{invoke, get, set, call}` is the API that a later Lua front-end binds to directly (§11). |
| Protocol | One JSON object per statement on stdout (JSON Lines). Each command is a transaction: on error or CANCELLED the state is rolled back, the stream continues, and every response carries Blender-style reports plus a `changes` delta (§12–§14). |
| Persistence | `.frap` is a deterministic zip holding `manifest.json`, `scene.json` (name-referenced, diffable) and content-addressed `buffers/` and `images/` (§16). |
| glTF | `frap-gltf` is a format-only crate: lossless serde model, GLB container, accessors, one file per extension. `frap-gltf-blender` maps the scene to and from glTF using io_scene_gltf2 5.2.41 semantics. `frap_*` properties unlock features Blender doesn't produce (§17). |
| Materials | One "material lowering" (`frap-material`) turns Blender shader node trees into a glTF-shaped `LoweredMaterial`, which **both** the exporter and the renderer consume. The importer does the inverse ("raising") (§18). |
| Renderer | `frap-render-scene` builds plain data with no GPU types, so it can be tested on its own. `frap-render` (wgpu 30, isolated) has two engines behind `trait Engine`: Workbench and EEVEE-like. View transforms are applied through 3D LUTs baked from Blender. There is also a `frap.render_contact_sheet` operator (§20). |
| Tests | Golden cases are bpy scripts run in both Blender and frap and dumped to one shared `GoldenState` JSON schema. They are compared canonically (exact element order only for ops listed as order-stable). Blender fixtures are checked in, so CI without Blender still compares against Blender truth (§21). |
| Milestones | M0 contracts → M1 object-mode loop → M2 static GLB and Workbench render → M3 edit mode and UV → M4 modifiers → M5 materials and EEVEE → M6 animation, skinning and morphs → M7 compression and extensions → M8 parity hardening. Each milestone has 5–9 work packages with explicit crate and directory ownership (§23). |

---

## 1. Buildability rules (normative for all contributors)

These rules are part of the architecture. Reviewers reject PRs that break them.

1. **One concept, one file.**
   - Each operator lives in its own file, for example `crates/frap-op-mesh/src/ops/extrude_region.rs`. So does each modifier evaluator (`crates/frap-modifiers/src/types/bevel.rs`), each glTF extension codec (`crates/frap-gltf/src/ext/khr_materials_sheen.rs`), each primitive generator, each RNA function hook, each built-in node group and each view transform.
   - Modules are pulled in with `automod::dir!("src/ops")`, and registration uses `#[linkme::distributed_slice]`.
   - **Nobody edits a `mod` list or a `register_all()` function.**
2. **Generated code is never hand-edited.**
   - Everything under `*/src/gen/` and all of `crates/frap-spec` is produced by `cargo xtask gen` from `schema/blender-5.2/*.json`.
   - Regeneration is its own PR, owned by the "spec" work package.
   - CI runs `cargo xtask gen --check` to fail the build when the generated code is out of date.
3. **Workspace dependencies are pre-declared.**
   - Every third-party crate the plan needs is in root `[workspace.dependencies]` from M0, with exact pins for the young crates.
   - Member crates write `foo.workspace = true`.
   - Adding a new external dependency needs the integrator's approval. This keeps `Cargo.toml` and `Cargo.lock` conflicts rare.
4. **Contracts freeze per milestone.**
   - Public traits and types marked `// CONTRACT` (listed in §3.3) change only through a short RFC in `docs/design/rfc/`, merged before dependent work starts.
   - Additive changes are fine at any time: new enum variants on `#[non_exhaustive]` enums, new methods with default implementations.
5. **Tests are colocated and per-file.**
   - Golden cases are one `.py` file each, with one fixture JSON each.
   - CLI transcripts are one `.trycmd` file each.
   - Snapshot tests (`insta`) are named after the file under test.
   - There is no shared "big test file".
6. **Stubs from day one.**
   - At the end of M0 every crate compiles, and every public function exists, returning `Err(NotImplemented)` or empty data.
   - Downstream work never waits on an upstream *implementation*, only on its *signature*.
7. **FFI budget.**
   - Allowed in default features: `meshopt` (vendored meshoptimizer via `cc`) and `webp` / `libwebp-sys` (vendored libwebp via `cc`).
   - Optional, off by default: `basisu_c_sys` (needs libclang).
   - Forbidden: cmake, `git clone` in `build.rs`, bindgen in default features.
   - A CI job builds the default feature set in a clean container that has only `cc`/`c++`.
8. **Young crates sit behind frap traits.** These crates are each wrapped by a trait owned by frap, pinned with `=x.y.z`, and covered by golden tests:

   | Young crate | frap trait | Wrapped in |
   |---|---|---|
   | `manifold-rust` | `BooleanSolver` | `frap-boolean` |
   | `opensubdiv-rs` | `Subdivider` | `frap-subdiv` |
   | `draco-core` | `DracoCodec` | `frap-codec-mesh` |

   Each one can be replaced without touching its callers.
9. **No `HashMap` iteration in any observable output.** Use `IndexMap`, `BTreeMap` or sorted vectors (§22).
10. **stdout belongs to the protocol.** Logs go to stderr through `tracing`. No crate except `frap-cli` writes to stdout.

---

## 2. Decisions taken here, and decisions left to the user

Taken by this proposal (they can be revisited, but the plan assumes them):

| # | Decision | Rationale |
|---|---|---|
| A1 | The initial state is **Blender's factory startup** (Cube, Camera, Light), the same as `blender -b --factory-startup`. `--empty` starts like `wm.read_homefile(use_empty=True)`. | Scripts and LLM habits assume the default cube. Golden parity is easier. |
| A2 | Floats print as **shortest f32** (`0.1`, not `0.10000000149011612`). | Values are f32 in Blender. Output is smaller and easier for LLMs to read. Goldens compare numbers, not text. |
| A3 | frap acts as if **one virtual 3D View** exists, with the factory *Layout* viewport matrix. | Matches the poll and behaviour that need a view [ops-object §1.7]. |
| A4 | "Smooth by Angle" is a **built-in NODES group** with Blender's identifiers (`Input_1`, `Socket_1`). Other node groups on a NODES modifier report "unsupported node group" and are skipped. | [rna §5.2] |
| A5 | Element order: the BMesh pool reproduces `BLI_mempool` LIFO reuse. Goldens compare canonically by default, and exactly for the order-stable ops in [ops-mesh §5.7]. | Exact order is possible but costly per op. Parity is earned op by op. |
| A6 | `render.engine='CYCLES'` is accepted. frap renders with the EEVEE-like engine and adds a WARNING report. | Agents often set CYCLES. Refusing would just make them retry. |
| A7 | Blender's 4.x-era paths (`action.fcurves`, `mod["Input_1"]`, `use_auto_smooth`, `BLENDER_EEVEE_NEXT`) produce errors whose message names the 5.x replacement. | LLM agents are trained mostly on 4.x scripts. |
| A8 | Datablock writes made while the mesh is in edit mode follow Blender: they are overwritten when leaving edit mode. A WARNING report says so. Reads are always fresh, because frap flushes edit → mesh before every read. | [rna §4] |

Left to the user (see §25 for consequences): **D1 license** (GPL-3.0-or-later vs permissive), **D2 Suzanne** (depends on D1), **D3 how strict default parity must be** for UV packing and bevel corners.

---

## 3. Cargo workspace and crate layout

### 3.1 Repository tree

```
frap/
├─ Cargo.toml                 # [workspace], [workspace.dependencies], profiles (integrator-owned)
├─ rust-toolchain.toml        # 1.97
├─ crates/
│  ├─ frap-math/              # L0 glam re-export, Blender euler/quat/matrix conventions, BliRng, f32 fmt
│  ├─ frap-macros/            # L0 proc-macros: #[operator], #[rna_func], #[rna_hook], rna_struct!, #[modifier]
│  ├─ frap-rna/               # L1 IdKey/IdRef/IdHeader, PropDef/StructDef/FuncDef, Value/Input, coerce, RnaPtr, traits
│  ├─ frap-geom/              # L1 polyfill+beautify, Newell normals, corner normals, kd-tree, mikktspace wrapper
│  ├─ frap-color/             # L1 view transforms (LUT application), colorspaces, sRGB/linear, dither
│  ├─ frap-spec/              # L2 GENERATED: operator props structs + StructDef tables + enums (from schema/)
│  ├─ frap-mesh/              # L3 Mesh (SoA attributes), primitives, looptris, normals, from_pydata, RNA bindings
│  ├─ frap-anim/              # L3 Action/slot/layer/strip/channelbag/FCurve/NLA data + pure evaluation
│  ├─ frap-nodes/             # L3 NodeTree/Node/Socket/Link, node type registry (generated socket tables)
│  ├─ frap-gltf/              # L3 glTF JSON model, GLB container, accessor codec, ext/* (format only)
│  ├─ frap-uv/                # L3 LSCM/ABF++/SLIM, projections, smart_project, packer (on ParamMesh)
│  ├─ frap-subdiv/            # L3 Subdivider trait + Catmull-Clark backend(s)
│  ├─ frap-boolean/           # L3 BooleanSolver trait + manifold-rust backend
│  ├─ frap-codec-image/       # L3 PNG/JPEG/WebP/HDR/EXR/KTX2 decode+encode behind features
│  ├─ frap-codec-mesh/        # L3 meshopt + Draco codecs behind traits/features
│  ├─ frap-bmesh/             # L4 BMesh, kernel Euler ops, bmesh.ops.*, Mesh<->BMesh conversion
│  ├─ frap-scene/             # L5 Main, IdMap, all ID datablocks, Scene/ViewLayer/Base, Context, EditState, Host trait
│  ├─ frap-modifiers/         # L6 ModifierTypeInfo registry + types/*.rs evaluators
│  ├─ frap-material/          # L6 node tree -> LoweredMaterial (lowering) and back (raising)
│  ├─ frap-eval/              # L7 depsgraph-lite: evaluated objects/meshes/poses, animation application
│  ├─ frap-op/                # L8 operator framework: OperatorDef, registry, OpCtx, poll lib, macro runner, reports
│  ├─ frap-render-scene/      # L8 evaluated scene -> RenderScene (plain data, no wgpu)
│  ├─ frap-gltf-blender/      # L8 scene <-> glTF with io_scene_gltf2 5.2.41 semantics
│  ├─ frap-project/           # L8 .frap zip read/write, schema versioning, migrations
│  ├─ frap-render/            # L9 wgpu device, Workbench + EEVEE-like engines, readback, contact sheet compositor
│  ├─ frap-op-object/         # L9 object.*, transform.* (object mode), view3d.*, collection ops
│  ├─ frap-op-mesh/           # L9 mesh.* (incl. primitives), transform.* edit-mode adapters, geometry.*
│  ├─ frap-op-uv/             # L9 uv.*
│  ├─ frap-op-anim/           # L9 anim.*, screen.frame_*, pose.*, armature.*, keyframe RNA funcs
│  ├─ frap-op-data/           # L9 material.*, image.*, world.*, bpy.data.*.new/remove RNA funcs, node funcs
│  ├─ frap-op-io/             # L9 import_scene.gltf, export_scene.gltf, wm.* file ops
│  ├─ frap-op-render/         # L9 render.render, frap.render_contact_sheet
│  ├─ frap-op-frap/           # L9 frap.* extension ops (describe, select_elements, scene_summary, debug_dump_state)
│  ├─ frap-ops-all/           # L9 aggregator: depends on every frap-op-*; `pub fn link()` defeats linker GC
│  ├─ frap-lang/              # L1* lexer, bpy-line parser, short-form parser -> IR (depends only on frap-rna Input)
│  ├─ frap-session/           # L10 Session, transactions, IR executor, path resolver, response builder
│  ├─ frap-schema/            # L10 --describe: manifest, JSON Schema, LLM tool formats
│  ├─ frap-cli/               # L11 binary `frap`
│  └─ frap-golden/            # L11 GoldenState schema, dumpers, canonicalisers, comparators, runner
├─ schema/blender-5.2/        # dumped from Blender (owned by WP0.2), plus frap-extras.json (python-only props)
├─ tools/blender/             # rna_dump.py, ops_dump.py, nodes_dump.py, startup_dump.py, golden_runner.py,
│                             # dump_state.py, bake_view_luts.py
├─ tests/golden/{cases,fixtures}/<area>/...
├─ tests/cli/*.trycmd
├─ assets/                    # baked LUTs, matcaps, studio lights, BRDF LUT (generated, checked in)
├─ xtask/                     # gen, gen --check, golden bless, luts, release
└─ docs/{research,design}/
```

### 3.2 Dependency layers (arrows point to dependencies)

```
L11  frap-cli ─────────┬─────────── frap-golden
L10  frap-session ─────┼─ frap-schema          (frap-lang is a leaf above frap-rna)
L9   frap-ops-all → frap-op-{object,mesh,uv,anim,data,io,render,frap}    frap-render
L8   frap-op   frap-render-scene   frap-gltf-blender   frap-project
L7   frap-eval
L6   frap-modifiers   frap-material
L5   frap-scene
L4   frap-bmesh
L3   frap-mesh frap-anim frap-nodes frap-gltf frap-uv frap-subdiv frap-boolean frap-codec-image frap-codec-mesh
L2   frap-spec
L1   frap-rna   frap-geom   frap-color   frap-lang
L0   frap-math   frap-macros(proc-macro)
```

Rules:

- A crate may depend only on lower layers.
- `frap-gltf`, `frap-uv`, `frap-subdiv`, `frap-boolean`, the codecs, `frap-geom` and `frap-color` do **not** depend on `frap-scene`. They work on their own small input types, so each can be developed and tested by one agent alone.
- `frap-render` depends on `frap-render-scene` *types* only, not on `frap-scene`. Render tests use synthetic `RenderScene`s.
- `frap-lang` depends only on `frap-rna::Input`. A Lua front-end would be a sibling crate (`frap-lua`) that calls `frap-session`.

### 3.3 Contract surfaces (frozen at the end of M0)

| Contract | Crate | Section |
|---|---|---|
| `IdKey`, `IdRef<T>`, `IdAny`, `IdCode`, `IdHeader` | frap-rna | §5 |
| `PropDef`, `StructDef`, `FuncDef`, `Value`, `Input`, `RnaError`, `coerce()` | frap-rna | §9 |
| `Rna`, `RnaMut`, `RnaColl`, `RnaCollMut`, `Field`, `RnaPtr`, `Seg`, `Root` | frap-rna | §9 |
| `Prop<T>`, `OpArgs`, `OpProps` | frap-rna | §10 |
| `Mesh`, `AttrData`, `Domain`, `AttrType` | frap-mesh | §6 |
| `BMesh` public API (iterators, kernel ops, `from_mesh`/`to_mesh`) | frap-bmesh | §6 |
| `Main`, `IdMap<T>`, `Host`, `Context`, `EditState`, `Evaluator` | frap-scene | §5, §8 |
| `ModifierTypeInfo`, `ModEvalCx` | frap-modifiers | §7 |
| `OperatorDef`, `OpCtx`, `OpStatus`, `OpError`, `Report`, `PollFail` | frap-op | §10 |
| `Stmt`, `Expr`, `Lit`, `Span` (the IR) | frap-lang | §11 |
| `Response` JSON shape (`schemars`-described) | frap-session | §13 |
| `LoweredMaterial` | frap-material | §18 |
| `RenderScene`, `Engine`, `Film` | frap-render-scene / frap-render | §20 |
| `GoldenState` JSON schema | frap-golden | §21 |

### 3.4 Feature flags (on the `frap-cli` and `frap-ops-all` facades)

| Feature | Default | Effect |
|---|---|---|
| `gpu` | on | Builds `frap-render`. Without it, `render.render` returns `NotImplemented("built without gpu")`. Lets parser and modelling CI skip wgpu. |
| `webp-lossy` | on | Lossy WebP via libwebp (`cc`). Without it, frap encodes WebP lossless only and adds a warning. |
| `meshopt` | on | `meshopt` crate (`cc`). |
| `draco` | on once the M7 spike passes | `draco-core`, pure Rust. |
| `draco-sys` | off | Fallback: Google Draco compiled with `cc` (no cmake). |
| `basisu` | on | `ktx2` + `basisu` transcoder (pure Rust), for import and render. |
| `basisu-encode` | off | `basisu_c_sys` (needs libclang). |
| `subdiv-osd` | on | `opensubdiv-rs` backend. Off falls back to frap's own Catmull-Clark. |
| `boolean-manifold` | on | `manifold-rust` backend. |

---

## 4. Code generation from Blender (single source of truth)

### 4.1 Dumps (WP0.2; regenerated whenever the Blender version changes)

All scripts run as `$BLENDER_BIN -b --factory-startup --python tools/blender/<x>.py -- --out schema/blender-5.2/`:

| Script | Output | Content |
|---|---|---|
| `ops_dump.py` | `ops/<namespace>.json` | For each operator in a curated namespace list: `bl_idname`, label, description, `bl_options`, and every property (identifier, type, subtype, unit, array shape, default, hard and soft range, enum items, `is_enum_flag`, `is_hidden`, `is_skip_save`, `is_required`, pointer `fixed_type` for macro sub-ops). Also includes the poll matrix A–E with and without a VIEW_3D override [ops-object §0]. |
| `rna_dump.py` | `rna/<Struct>.json` | `bl_rna` for every struct reachable from `bpy.types` that frap might implement: properties, functions (params, returns, keyword-only markers), base struct. Includes **instance defaults** read from fresh datablocks, and records whenever they differ from RNA defaults [rna §0]. |
| `nodes_dump.py` | `nodes/<bl_idname>.json` | Per node: inputs and outputs (`name`, `identifier`, socket type, default, `enabled` under each relevant mode, `hide_value`), node properties, legacy `type` string. |
| `modifiers_dump.py` | `modifiers/<TYPE>.json` | Fresh values from `modifiers.new`, default names from `modifier_add` [rna §5.1]. |
| `startup_dump.py` | `startup.json` | The complete factory startup state, including the default cube's exact mesh arrays. Its vertex order differs from `primitive_cube_add` [ops-mesh §1.4]. |
| `gltf_dump.py` | `ops/export_scene.json`, `ops/import_scene.json` | All 110 and 20 properties. Dynamic enum supersets are recorded by hand in `schema/overrides.toml`. |

`schema/overrides.toml` holds what introspection cannot see:

- dynamic enum supersets (`render.engine`, `export_format`, `mode_set.mode`);
- Python-only members (`Object.children`, `users_collection`, `Mesh.from_pydata`, `shade_smooth`, `shade_flat`);
- frap notes (`frap_note`: "default offset=0 is a no-op", "location defaults to 3D cursor when unset");
- headless deviations (`loopcut` works from `edge_index`; `spin` needs `axis`).

### 4.2 What `cargo xtask gen` produces

| Output | Location | Hand-written counterpart |
|---|---|---|
| `OperatorSpec` statics plus one typed props struct per operator, for example `frap_spec::ops::mesh::PrimitiveCubeAdd` (example below) | `crates/frap-spec/src/gen/ops/<ns>.rs` | `#[operator]` exec function in a `frap-op-*` crate |
| `StructDef` statics for every dumped struct (props, funcs, base, refine info) | `crates/frap-spec/src/gen/rna/*.rs` | `Rna` bindings in the data crates |
| **Plain data structs with RNA bindings**: Camera, Light (+Point/Sun/Spot/Area), RenderSettings, ImageFormatSettings, ColorManagedViewSettings, View3DShading, SceneDisplay, SceneEEVEE, ToolSettings (subset), UnitSettings, View3DCursor, every modifier settings struct, CameraDOFSettings, Keyframe, NlaStrip fields… | `crates/frap-scene/src/gen/plain/*.rs`, `crates/frap-anim/src/gen/*.rs` | none: fields, `Default` (fresh values), serde and `Rna` all generated |
| Node type table (`NodeTypeDef` per `bl_idname`) | `crates/frap-nodes/src/gen/types/*.rs` | `update_sockets` functions for nodes whose socket availability depends on mode (Mix, Math, Separate/Combine Color…) in `crates/frap-nodes/src/dynamic/*.rs` |
| `ModifierType` enum, `ModifierSettings` enum of generated structs | `crates/frap-scene/src/gen/modifier_settings.rs` | evaluators in `frap-modifiers/src/types/*.rs` |
| glTF exporter and importer settings structs (110 / 20 props) | `crates/frap-spec/src/gen/ops/export_scene.rs` | `frap-gltf-blender` |

The plain-struct codegen does most of the volume reduction. It covers several hundred properties with correct names, types, ranges and *fresh* defaults. No agent has to type them, and no agent can typo them.

The generated props struct for `mesh.primitive_cube_add`:

```rust
// crates/frap-spec/src/gen/ops/mesh.rs   (GENERATED — do not edit)
pub mod primitive_cube_add {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Align { World, View, Cursor }               // generated from enum items
    impl frap_rna::EnumIdent for Align { /* "WORLD" <-> World ... */ }
}
#[derive(Debug, Clone)]
pub struct PrimitiveCubeAdd {
    pub size: Prop<f32>,                // default 2.0, hard [0, inf], DISTANCE
    pub calc_uvs: Prop<bool>,           // default true
    pub enter_editmode: Prop<bool>,     // hidden-ish flag in UI; settable from Python
    pub align: Prop<primitive_cube_add::Align>,
    pub location: Prop<[f32; 3]>,       // TRANSLATION; unset => 3D cursor
    pub rotation: Prop<[f32; 3]>,       // EULER; unset => from align
    pub scale: Prop<[f32; 3]>,          // hidden; sentinel (0,0,0) => unset
}
impl OpProps for PrimitiveCubeAdd {
    const IDNAME: &'static str = "mesh.primitive_cube_add";
    const BL_IDNAME: &'static str = "MESH_OT_primitive_cube_add";
    fn spec() -> &'static OperatorSpec { &OPSPEC_MESH_OT_PRIMITIVE_CUBE_ADD }
    fn from_args(a: &OpArgs) -> Self { /* generated field-by-field extraction */ }
}
```

### 4.3 Drift tests

- `frap-spec` has a test that loads the JSON and compares it with the compiled tables, catching stale generation.
- Every crate with hand-written `Rna` bindings runs `frap_rna::testing::check_bindings::<T>()`. It asserts that:
  - every bound property exists in the `StructDef` with the same type, array shape and enum items;
  - every property in the "implemented subset" list `schema/implemented.toml` has a binding.

---
## 5. Core scene data model (`frap-rna::id`, `frap-scene`)

### 5.1 Identity and references

```rust
// frap-rna/src/id.rs   // CONTRACT
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub struct IdKey(NonZeroU64);              // session-unique, never persisted, never reused

#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub enum IdCode { OB, ME, MA, CA, LA, WO, IM, AR, AC, SC, GR, KE, NT, TE }  // Blender 2-letter codes

pub struct IdRef<T: IdType>(IdKey, PhantomData<fn() -> T>);   // typed pointer between datablocks
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct IdAny { pub code: IdCode, pub key: IdKey }

pub trait IdType: Rna + Clone + Send + Sync + 'static {
    const CODE: IdCode;
    const DEFAULT_NAME: &'static str;               // "Mesh", "Material", ...
    fn header(&self) -> &IdHeader;
    fn header_mut(&mut self) -> &mut IdHeader;
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)); // used by users-count, orphan purge, save, deps
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>); // used by remove(do_unlink), user_remap
}

pub struct IdHeader {
    pub name: SmolStr,          // <= 255 bytes UTF-8, unique within its IdMap
    pub use_fake_user: bool,
    pub gen: u64,               // bumped on every mutable access (see §5.3)
    pub anim: Option<AnimData>, // frap-anim::AnimData (action, slot handle, NLA); every animatable ID
    pub custom: IndexMap<SmallStr, IdProp>, // ID properties -> glTF extras when export_extras
}
```

Pointers between datablocks are always `IdRef<T>`. They are never names and never `Arc`s. That way renaming is O(1), and rollback cannot leave dangling `Arc`s. References persist as `(code, name)` (§16).

### 5.2 `Main` (= `bpy.data`)

```rust
// frap-scene/src/main.rs   // CONTRACT
pub struct Main {
    pub objects: IdMap<Object>,   pub meshes: IdMap<Mesh>,       pub materials: IdMap<Material>,
    pub cameras: IdMap<Camera>,   pub lights: IdMap<Light>,      pub worlds: IdMap<World>,
    pub images: IdMap<Image>,     pub armatures: IdMap<Armature>, pub actions: IdMap<Action>,
    pub scenes: IdMap<Scene>,     pub collections: IdMap<Collection>, pub shape_keys: IdMap<Key>,
    pub node_groups: IdMap<NodeTree>, pub textures: IdMap<Texture>,
    pub filepath: Option<Utf8PathBuf>, pub is_dirty: bool,
}

pub struct IdMap<T: IdType> {
    items: IndexMap<IdKey, Arc<T>>,     // iteration = Blender ID-list order (kept name-sorted)
    by_name: BTreeMap<SmolStr, IdKey>,
}
impl<T: IdType> IdMap<T> {
    pub fn get(&self, r: IdRef<T>) -> Option<&T>;
    pub fn get_mut(&mut self, r: IdRef<T>, log: &mut ChangeLog) -> Option<&mut T>; // Arc::make_mut + gen bump + log
    pub fn find(&self, name: &str) -> Option<IdRef<T>>;
    pub fn index_of(&self, r: IdRef<T>) -> Option<usize>;      // bpy.data.objects.find()
    pub fn at(&self, i: isize) -> Option<IdRef<T>>;             // bpy.data.objects[-1]
    pub fn insert(&mut self, requested: &str, value: T, log: &mut ChangeLog) -> IdRef<T>; // unique_name applied
    pub fn rename(&mut self, r: IdRef<T>, requested: &str, mode: RenameMode, log: &mut ChangeLog) -> RenameResult;
    pub fn remove(&mut self, r: IdRef<T>, log: &mut ChangeLog) -> Option<Arc<T>>;
    pub fn iter(&self) -> impl Iterator<Item = (IdRef<T>, &T)>;
}
```

- **Naming** lives in `frap-rna::naming` as one function, `unique_name(existing: &dyn Fn(&str) -> bool, requested: &str, max_bytes: usize) -> SmolStr`. It implements the rules in [rna §0.1–0.2]:
  - truncation on a UTF-8 character boundary;
  - split at the last `.NNN`;
  - take the lowest free suffix ≥ 1, zero-padded to three digits;
  - fall back to the type default when the name is empty.

  ID names allow 255 bytes and sub-data names 63. One table-driven test file holds Blender's verified cases.
- **Order:** `bpy.data.<coll>[i]` follows Blender's sorted ID list. Whether the comparison is case-sensitive or natural is checked by a golden case in WP1.1 before it is frozen.
- **User counts** are computed on demand by scanning `for_each_ref` over all IDs. They are not maintained incrementally. This is O(#IDs). frap scenes are small, so it is fast enough, and it cannot drift. Orphan purge on save drops IDs with 0 users and no fake user [ops-object §1.11].

### 5.3 Copy-on-write, generations, change log

- Every datablock sits in an `Arc<T>`. Mutable access goes through `IdMap::get_mut`. That call does three things:
  1. `Arc::make_mut`, which clones only that datablock, and only if a snapshot still shares it;
  2. sets `header.gen = GEN.fetch_add(1)`;
  3. records `IdAny` in the per-command `ChangeLog` (`modified`).
- `GEN` is a process-global `AtomicU64`. It lives **outside** any snapshot, so a rollback can never reuse a generation number. Evaluation caches are keyed by generation (§7.3).
- Heavy arrays inside a datablock are `Arc<[T]>` / `Arc<Vec<T>>` as well. Mesh attribute buffers are an example. Changing `obj.location` therefore copies an `Object` struct of about 1 KB, never the mesh.
- `ChangeLog { added: Vec<IdAny>, removed: Vec<IdAny>, modified: IndexSet<IdAny>, context_changed: bool }` feeds the `changes` field of every response (§13).

### 5.4 Datablock types (one file each in `frap-scene/src/id/`)

| Type | File | Key content (Blender names) |
|---|---|---|
| `Object` | `object.rs` (hand-written; the most complex) | `data: Option<IdAny>` (fixed at creation, decides `type`), `location`, `rotation_mode`, `rotation_euler`/`_quaternion`/`_axis_angle`, `scale`, `delta_*`, `parent`, `parent_type`, `parent_bone`, `matrix_parent_inverse`, `modifiers: Vec<Modifier>`, `constraints` (P2), `vertex_groups: Vec<VertexGroup>`, `material_slots: Vec<MaterialSlot{link, material}>`, `active_material_index`, `pose: Option<Pose>`, `mode: ObjectMode`, `hide_viewport/render/select`, `display_type`, `color`, `empty_display_type/size`, `active_shape_key_index`, `show_*` |
| `Mesh` | in `frap-mesh` (§6.1) | |
| `Material` | `material.rs` | `node_tree: NodeTree` (embedded, always present), `surface_render_method`, `blend_method` (deprecated mirror), `use_backface_culling`, `diffuse_color`, `metallic`, `roughness`, `alpha_threshold`, `pass_index`, … |
| `Camera`, `Light`, `World` | `gen/plain/*` + `camera.rs`/`light.rs`/`world.rs` for the hooks (`angle`↔`lens`, `light.type` re-typing the struct) | |
| `Image` | `image.rs` | `source`, `filepath`, `packed: Option<Arc<EncodedImage{bytes, mime}>>`, `generated_*`, `colorspace_settings.name`, `alpha_mode`, `size`, `pixels` (decoded lazily, cached in runtime) |
| `Armature` | `armature.rs` | `bones: Vec<Bone>` (rest), `collections`, `pose_position`, `display_type` |
| `Action` | in `frap-anim` | slots, layers, strips, channelbags, F-curves [rna §13] |
| `Key` | `key.rs` | `key_blocks: Vec<ShapeKey{name, value, slider_min/max, relative_key, vertex_group, mute, interpolation, data: Arc<Vec<[f32;3]>>}>`, `use_relative`, `reference_key` |
| `Scene` | `scene.rs` | `collection` (master, embedded), `view_layers: Vec<ViewLayer>`, `camera`, `world`, `frame_*`, `render: RenderSettings`, `view_settings`, `display_settings`, `display` (Workbench), `eevee`, `cursor`, `tool_settings`, `unit_settings`, glTF variant settings (M7) |
| `Collection` | `collection.rs` | `objects: Vec<IdRef<Object>>`, `children: Vec<IdRef<Collection>>`, `hide_*`, `color_tag` |
| `NodeTree` (group) | in `frap-nodes` | "Smooth by Angle" and "glTF Material Output" built-in groups |
| `Texture` | `texture.rs` | only what the DISPLACE modifier needs (`IMAGE`, `NONE` first) |

### 5.5 Factory startup

`frap-scene::startup::factory(&mut Main) -> WindowState` builds the factory state from `schema/blender-5.2/startup.json`. That data is compiled in with `include_str!`, which avoids depending on a runtime file.

`startup::empty()` is the `use_empty=True` variant. `wm.read_homefile` and `wm.read_factory_settings` call these.

---

## 6. Mesh representation

### 6.1 `frap-mesh::Mesh`: storage and object mode (mirrors Blender 5.2 [ops-mesh §1.2])

```rust
// crates/frap-mesh/src/mesh.rs   // CONTRACT
pub struct Mesh {
    pub header: IdHeader,
    pub face_offsets: Arc<Vec<i32>>,          // len = faces + 1  (polygons[i].loop_start/total)
    pub attributes: AttributeSet,             // includes "position", ".edge_verts", ".corner_vert", ".corner_edge"
    pub materials: Vec<Option<IdRef<Material>>>,
    pub deform_verts: Option<Arc<Vec<DeformVert>>>, // MDeformVert: SmallVec<[(u32 /*group*/, f32); 4]>
    pub vertex_group_names: Vec<SmolStr>,     // 5.x location verified in WP6.3; Object.vertex_groups views it
    pub shape_keys: Option<IdRef<Key>>,
    pub uv_meta: UvMeta,                      // active / active_render UV layer names
    pub color_meta: ColorMeta,                // active_color_name, default_color_name
    pub texspace: TexSpace,
    #[doc(hidden)] pub runtime: MeshRuntime,  // NOT persisted, NOT compared: OnceLock caches
}
pub struct AttributeSet { attrs: Vec<Attribute> }       // Blender order: by domain then creation
pub struct Attribute { pub name: SmolStr, pub domain: Domain, pub data: AttrData }
pub enum Domain { Point, Edge, Face, Corner }
pub enum AttrData {                                     // each Arc'd for COW
    Float(Arc<Vec<f32>>), Int(Arc<Vec<i32>>), Int8(Arc<Vec<i8>>), Bool(Arc<Vec<bool>>),
    Float2(Arc<Vec<[f32; 2]>>), Float3(Arc<Vec<[f32; 3]>>), Float4(Arc<Vec<[f32; 4]>>),
    FloatColor(Arc<Vec<[f32; 4]>>), ByteColor(Arc<Vec<[u8; 4]>>),
    Int16x2(Arc<Vec<[i16; 2]>>), Int32x2(Arc<Vec<[i32; 2]>>),
    Quaternion(Arc<Vec<[f32; 4]>>), Float4x4(Arc<Vec<[[f32; 4]; 4]>>), String(Arc<Vec<SmolStr>>),
}
pub struct MeshRuntime {
    pub corner_normals: OnceLock<Arc<Vec<[f32; 3]>>>,  // lnor spaces, sharp edges/faces, custom_normal
    pub face_normals: OnceLock<Arc<Vec<[f32; 3]>>>,
    pub vert_normals: OnceLock<Arc<Vec<[f32; 3]>>>,
    pub looptris: OnceLock<Arc<Vec<[u32; 3]>>>,        // Blender looptri rule (polyfill for n-gons)
    pub bounds: OnceLock<Option<[[f32; 3]; 2]>>,
}
```

- Proxy RNA properties (`MeshPolygon.use_smooth` ↔ `!sharp_face`, `MeshEdge.use_seam` ↔ `uv_seam`, `use_edge_sharp`) create attributes lazily and remove them when they become all-false, exactly as Blender does [ops-mesh §2.5].
- `frap-mesh/src/primitives/*.rs` holds one generator per primitive, returning `Mesh`. Each reproduces Blender's exact vertex, edge, face and UV order. Order is verified against fixtures dumped from Blender in WP1.4.
- `frap-mesh/src/tessellate.rs` holds `looptris()` (the Blender rule). `frap-mesh/src/split.rs` holds `corner_split()`, which splits vertices the way the glTF exporter and the renderer both need [gltf §4.3].
- RNA bindings for `vertices`, `edges`, `polygons`, `loops`, `loop_triangles`, `attributes`, `uv_layers`, `color_attributes`, `vertex_normals`, `polygon_normals`, `corner_normals` live in `frap-mesh/src/rna/*.rs`, one file per element struct.

### 6.2 `frap-bmesh::BMesh`: the edit-mode structure

```rust
// crates/frap-bmesh/src/types.rs   // CONTRACT
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)] pub struct VertId(u32);
// EdgeId, LoopId, FaceId likewise. Ids are pool *slots*; iteration order == slot order.

pub struct Pool<T> { slots: Vec<Option<T>>, free: Vec<u32>, live: u32 }  // free = LIFO stack (BLI_mempool)
impl<T> Pool<T> {
    pub fn alloc(&mut self, v: T) -> u32;   // pop free (most recently freed first) else push
    pub fn free(&mut self, i: u32);         // push onto free
    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)>; // ascending slot, skipping free
}

pub struct BMVert { pub co: Vec3, pub no: Vec3, pub e: Option<EdgeId>, pub flag: ElemFlag }
pub struct BMEdge { pub v: [VertId; 2], pub l: Option<LoopId>, pub disk: [DiskLink; 2], pub flag: ElemFlag }
pub struct DiskLink { pub next: EdgeId, pub prev: EdgeId }
pub struct BMLoop { pub v: VertId, pub e: EdgeId, pub f: FaceId,
                    pub next: LoopId, pub prev: LoopId, pub radial_next: LoopId, pub radial_prev: LoopId }
pub struct BMFace { pub l_first: LoopId, pub len: u32, pub no: Vec3, pub mat_nr: i16, pub flag: ElemFlag }
bitflags! { pub struct ElemFlag: u16 { const SELECT=1; const HIDDEN=2; const SMOOTH=4; const SEAM=8;
                                       const TAG=16; const INTERNAL_TAG=32; } }

pub struct BMesh {
    pub verts: Pool<BMVert>, pub edges: Pool<BMEdge>, pub loops: Pool<BMLoop>, pub faces: Pool<BMFace>,
    pub vdata: CustomData, pub edata: CustomData, pub ldata: CustomData, pub pdata: CustomData, // columns by slot
    pub select_mode: SelectMode,            // VERT|EDGE|FACE bits (tool_settings.mesh_select_mode)
    pub select_history: Vec<BMElem>,        // last = active element
    pub act_face: Option<FaceId>,
    pub shape_layers: Vec<ShapeLayer>,      // one per key block while editing
}
```

The API is split into layers, each a separate module so that separate agents can own them:

| Module | Content | Naming |
|---|---|---|
| `kernel.rs` | `vert_create`, `edge_create`, `face_create`, `vert_kill`, `edge_kill`, `face_kill`, `split_edge_make_vert` (SEMV), `split_face_make_edge` (SFME), `join_edge_kill_vert` (JEKV), `join_face_kill_edge` (JFKE), `vert_separate`, `loop_reverse` | Blender's `bmesh_kernel_*` / `BM_*` names, so ports are 1:1 and creation order matches |
| `query.rs` | iterators (`vert_edges`, `edge_loops`, `face_loops`), `edge_exists`, `face_exists`, manifold tests | |
| `select.rs` | `select_flush_mode`, `select_set`, history ops, mode conversion [ops-mesh §2.2, §3.3] | |
| `customdata.rs` | typed layer columns, interpolation (`BM_loop_interp_from_face`, `BM_data_interp_from_verts`) | |
| `convert.rs` | `BMesh::from_mesh(&Mesh, &FromMeshParams)`, `to_mesh(&self, &Mesh /*template*/) -> Mesh` | mirrors `BM_mesh_bm_from_me` / `BM_mesh_bm_to_me`, including all-false attribute removal |
| `ops/<name>.rs` | **one file per `bmesh.ops` operator**, named and slotted as in [ops-mesh App. C]: `extrude_face_region`, `inset_region`, `bevel`, `subdivide_edges`, `remove_doubles`, `bridge_loops`, `triangulate`, `poke`, `dissolve_*`, `spin`, `solidify`, `wireframe`, `symmetrize`, `recalc_face_normals`, `triangle_fill`, `grid_fill`, `collapse`, … | |
| `validate.rs` | `validate(&BMesh) -> Result<(), Vec<Corruption>>`: disk and radial cycle consistency, loop counts | `proptest` uses it after random op sequences |

Every bmesh op has the signature `pub fn extrude_face_region(bm: &mut BMesh, i: &ExtrudeFaceRegionIn) -> ExtrudeFaceRegionOut`, with geometry slots typed as `Vec<VertId>`/`Vec<EdgeId>`/`Vec<FaceId>`. The edit-mode `mesh.*` operators are thin adapters: they gather the selection, call the bmesh op, update selection and history, and report counts. The two layers can therefore be built by different agents.

### 6.3 Original vs evaluated

| Situation | Source of truth | Readers |
|---|---|---|
| Object mode | `Main.meshes[m]` (`Mesh`) | RNA reads; evaluation input |
| Edit mode | `EditState.meshes[m]` (`Arc<BMesh>`) | RNA reads of `Mesh` first call `Session::flush_edit(m)`, which writes `to_mesh` into `Main` without leaving edit mode. Edit ops use the BMesh directly. |
| Evaluated | `frap-eval` cache: `Arc<Mesh>` per object | renderer, glTF exporter with `export_apply`, `dimensions`, `evaluated_get`, `to_mesh` |

---

## 7. Modifier evaluation pipeline (`frap-modifiers`, `frap-eval`)

### 7.1 Modifier data

`Object.modifiers: Vec<Modifier>`:

```rust
pub struct Modifier {                         // frap-scene/src/id/modifier.rs
    pub name: SmolStr,                        // <= 63 bytes, unique per object
    pub show_viewport: bool, pub show_render: bool, pub show_in_editmode: bool, pub show_on_cage: bool,
    pub show_expanded: bool, pub is_active: bool, pub use_pin_to_last: bool, pub persistent_uid: u32,
    pub settings: ModifierSettings,           // GENERATED enum: Subsurf(SubsurfSettings) | Bevel(..) | Nodes(..) | ...
}
```

`Modifier::srna()` refines the struct to `SubsurfModifier`, `BevelModifier` and so on, so the generated property tables apply. NODES settings hold `node_group: Option<IdRef<NodeTree>>` and `properties.inputs: IndexMap<SmolStr /*identifier*/, GnInput{name, value, type, attribute_name}>`, giving the 5.2 path `mod.properties.inputs.Input_1.value`.

### 7.2 Modifier plugins

```rust
// crates/frap-modifiers/src/lib.rs   // CONTRACT
pub enum ModKind { OnlyDeform, Constructive, Nonconstructive }
pub struct ModifierTypeInfo {
    pub ty: ModifierType,                          // generated enum
    pub kind: ModKind,
    pub flags: ModTypeFlags,                       // SUPPORTS_EDITMODE, ENABLE_IN_EDITMODE, ACCEPTS_CVS...
    pub depends_on: fn(&ModifierSettings) -> SmallVec<[IdAny; 2]>,   // boolean operand, armature, texture...
    pub eval: fn(&Modifier, Mesh, &mut dyn ModEvalCx) -> Result<Mesh, ModError>,
}
#[linkme::distributed_slice] pub static MODIFIER_TYPES: [ModifierTypeInfo];

pub trait ModEvalCx {                              // implemented by frap-eval
    fn object(&self) -> &Object;                   // the owner (vertex groups, matrix_world)
    fn evaluated_mesh(&mut self, ob: IdRef<Object>) -> Result<Arc<Mesh>, ModError>; // operands
    fn object_matrix_world(&mut self, ob: IdRef<Object>) -> Mat4;
    fn pose(&mut self, arm: IdRef<Object>) -> Result<Arc<EvaluatedPose>, ModError>;
    fn image(&mut self, im: IdRef<Image>) -> Option<Arc<DecodedImage>>;
    fn mode(&self) -> EvalMode;                    // Viewport | Render (show_viewport vs show_render)
    fn report(&mut self, r: Report);
    fn subdivider(&self) -> &dyn Subdivider; fn boolean_solver(&self) -> &dyn BooleanSolver;
}

// crates/frap-modifiers/src/types/mirror.rs  — the whole file an agent owns
#[modifier(MIRROR, kind = Constructive, flags = SUPPORTS_EDITMODE | ENABLE_IN_EDITMODE)]
fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let s = md.settings.as_mirror();               // generated accessor
    /* ... */
}
```

When a type has no registered evaluator, frap returns the input mesh unchanged and adds a WARNING report: `Modifier 'Remesh' (REMESH) is not evaluated by frap; passed through`. The response marks it, and `--describe` lists the support status of every modifier type.

### 7.3 Evaluation (depsgraph-lite)

```rust
// crates/frap-eval/src/lib.rs
pub struct EvalCache { objects: HashMap<(IdKey, EvalMode), CacheEntry> /* never iterated for output */ }
struct CacheEntry { fingerprint: u64, result: Arc<EvaluatedObject> }
pub struct EvaluatedObject {
    pub matrix_world: Mat4, pub mesh: Option<Arc<Mesh>>, pub bounds: Option<[[f32;3];2]>,
    pub pose: Option<Arc<EvaluatedPose>>, pub warnings: Vec<Report>,
}
impl frap_scene::Evaluator for EvalCache {
    fn object(&mut self, host: &dyn HostRead, ob: IdRef<Object>, mode: EvalMode) -> Result<Arc<EvaluatedObject>, EvalError>;
}
```

- **Fingerprint:** a hash of the object's `gen`, the `gen` of its data, the `gen`s of its shape keys, the active edit BMesh (if any), and recursively the fingerprints of its parent and of every `depends_on` target. Recursion is cycle-guarded, and a cycle reports an error. Because every write bumps a `gen`, invalidation is automatic. There is no tagging API to forget to call.
- **Mesh pipeline per object:**
  1. Start from the base mesh: the edit BMesh converted, or the original `Mesh`.
  2. Mix shape keys (relative mix at the current values).
  3. Run modifiers in stack order, with pinned-to-last modifiers moved to the end. Skip a modifier if `!show_viewport` (or `!show_render` in Render mode), or if the object is in edit mode and `!show_in_editmode`.
  4. Recompute normals lazily.
- **Animation is not part of evaluation.** As in Blender, `scene.frame_set(f)` and `frame_current = f` evaluate F-curves and NLA and write the animated values into the **original** data through RNA paths (§19). The `gen` bumps from those writes invalidate the cache.
- `Object.dimensions`, `matrix_world` (read), `evaluated_get(depsgraph)`, `to_mesh()` and `bpy.context.evaluated_depsgraph_get()` all route here. Reads always evaluate as needed, so they are never stale. This is a deliberate improvement over Blender [rna §0.11].

---

## 8. Context, selection and modes (`frap-scene/src/context.rs`)

```rust
pub struct WindowState {                      // part of the snapshot (§12)
    pub scene: IdRef<Scene>,
    pub view_layer: SmolStr,
    pub view3d: VirtualView3D,                // factory Layout viewport matrix + space_data.shading
}
pub struct ViewLayer {                        // inside Scene
    pub name: SmolStr,
    pub bases: Vec<Base>,                     // base order = Blender's layer-collection sync order
    pub active: Option<IdRef<Object>>,        // can be unselected
    pub active_layer_collection: LayerCollectionPath,
    pub layer_collection: LayerCollection,    // exclude / hide_viewport / holdout tree
}
pub struct Base { pub object: IdRef<Object>, pub flag: BaseFlag /* SELECTED | HIDDEN */ }

pub struct EditState {                        // part of the snapshot
    pub meshes: BTreeMap<IdKey /*Mesh*/, Arc<BMesh>>,
    pub armatures: BTreeMap<IdKey /*Armature*/, Arc<EditArmature>>,
}

pub struct Context<'a> { pub main: &'a Main, pub win: &'a WindowState, pub edit: &'a EditState, pub prefs: &'a Preferences }
impl<'a> Context<'a> {
    pub fn scene(&self) -> &'a Scene;                    pub fn view_layer(&self) -> &'a ViewLayer;
    pub fn active_object(&self) -> Option<IdRef<Object>>; pub fn object(&self) -> Option<IdRef<Object>>;
    pub fn selected_objects(&self) -> Vec<IdRef<Object>>; // base order, not click order
    pub fn selected_editable_objects(&self) -> Vec<IdRef<Object>>;
    pub fn visible_objects(&self) -> Vec<IdRef<Object>>;
    pub fn mode(&self) -> ContextMode;                    // OBJECT | EDIT_MESH | EDIT_ARMATURE | POSE | PAINT_WEIGHT
    pub fn edit_object(&self) -> Option<IdRef<Object>>;
    pub fn objects_in_mode(&self) -> Vec<IdRef<Object>>;
    pub fn objects_in_mode_unique_data(&self) -> Vec<IdRef<Object>>;
    pub fn collection(&self) -> IdRef<Collection>;        // where new objects are linked
    pub fn tool_settings(&self) -> &'a ToolSettings;      pub fn cursor(&self) -> &'a View3DCursor;
    pub fn active_bone(&self) -> Option<BoneRef>;         pub fn selected_pose_bones(&self) -> Vec<BoneRef>;
}
```

- `Object.mode` is stored on each object, and `context.mode` is derived from it.
- `object.mode_set(mode='EDIT')` enters multi-object edit: every selected object of the active object's type, deduplicated by data. For each one it builds a BMesh (`from_mesh`) into `EditState`. Leaving edit mode writes each BMesh back with `to_mesh` and drops it.
- `.frap` saves `Object.mode`. On load, an object saved in edit mode re-enters it, as Blender does.
- `bpy.context` is read-only. Writes go through `view_layer.objects.active = ...`, `obj.select_set()` and `obj.hide_set()`, all RNA functions or hooks. Assigning to a context member produces Blender's `AttributeError` wording.
- `bpy.context.space_data` resolves to the virtual 3D view stub (`shading`, `region_3d.view_matrix`). Everything else in the UI category returns `None` [ops-object §2.1].
- Each operator declares its poll as a function, built from a shared poll library (`frap-op/src/poll.rs`). Names follow Blender's `ED_operator_*`: `objectmode`, `editmesh`, `active_object`, `active_editable_mesh`, `scene_editable`, `posemode`, `editarmature`. Failure messages are Blender's verbatim, for example `Operator bpy.ops.mesh.bevel.poll() failed, context is incorrect`.

---
## 9. RNA-like property path system (`frap-rna`)

### 9.1 Metadata

```rust
// crates/frap-rna/src/def.rs   // CONTRACT
pub struct StructDef {
    pub identifier: &'static str,              // "Object", "BevelModifier", "ShaderNodeBsdfPrincipled"
    pub base: Option<&'static str>,            // "Modifier", "ID", "Node"
    pub name: &'static str, pub description: &'static str,
    pub props: &'static [PropDef],             // own props only; lookup walks base chain
    pub funcs: &'static [FuncDef],
    pub is_id: bool,
    pub frap: FrapMeta,                        // implemented? notes? python_only?
}
pub struct PropDef {
    pub identifier: &'static str, pub name: &'static str, pub description: &'static str,
    pub kind: PropKind, pub subtype: Subtype, pub unit: Unit,
    pub array: ArrayShape,                     // Scalar | Fixed(n) | Matrix(r, c) | Dynamic
    pub flags: PropFlags,                      // READONLY HIDDEN SKIP_SAVE ENUM_FLAG REQUIRED OUTPUT
                                               // PYTHON_ONLY DYNAMIC_ENUM NEVER_NULL KEYWORD_ONLY
    pub frap: FrapMeta,
}
pub enum PropKind {
    Bool  { default: Defaults<bool> },
    Int   { default: Defaults<i64>, hard: (i64, i64), soft: (i64, i64) },
    Float { default: Defaults<f64>, hard: (f64, f64), soft: (f64, f64), precision: u8, storage: FloatStorage /*F32|F64*/ },
    String{ default: &'static str, maxlen: u32 /*0 = unlimited*/ },
    Enum  { items: &'static [EnumItem], default: EnumDefault, dynamic: Option<DynEnumKey> },
    Pointer    { fixed_type: &'static str, is_id: bool },
    Collection { item_type: &'static str, srna: Option<&'static str> /* "ObjectModifiers": holds new/remove */ },
}
pub struct FuncDef {
    pub identifier: &'static str, pub description: &'static str,
    pub params: &'static [PropDef],            // order = positional order
    pub returns: &'static [PropDef],           // 0, 1, or tuple
    pub positional_only: u8, pub keyword_only_from: Option<u8>, // keyframe_insert(data_path, *, ...)
    pub flags: FuncFlags,                      // NO_SELF (collection-level), USE_SELF_ID, PYTHON_ONLY
}
```

### 9.2 Values in and out

Front-ends produce untyped **`Input`**. Typed **`Value`** comes out of coercion and getters. Keeping the two apart is what lets the parsers be built without the scene.

```rust
// crates/frap-rna/src/value.rs   // CONTRACT
pub enum Input {                               // what a front-end can express
    None, Bool(bool), Int(i64), Float(f64), Str(String),
    Tuple(Vec<Input>), List(Vec<Input>), Set(Vec<Input>), Dict(IndexMap<String, Input>),
    Math(MathCtor),                            // Vector((..)), Euler((..),'XZY'), Quaternion, Matrix, Color
    Ptr(RnaPtr),                               // result of evaluating a path expression
}
pub enum Value {                               // what the engine stores / returns
    None, Bool(bool), Int(i64), Float(f32), Double(f64), Str(SmolStr),
    Enum(SmolStr), EnumSet(BTreeSet<SmolStr>),
    BoolArray(SmallVec<[bool; 4]>), IntArray(SmallVec<[i64; 4]>),
    FloatArray { data: SmallVec<[f32; 16]>, shape: ArrayShape, tag: MathTag }, // tag: Vector|Euler(order)|Quaternion|AxisAngle|Matrix|Color|None
    Ptr(RnaPtr), Coll(RnaPtr), Tuple(Vec<Value>),
}
pub fn coerce(def: &PropDef, input: Input, cx: &CoerceCx) -> Result<Value, RnaError>;
```

`coerce` is the **only** place where Blender's assignment rules live [rna §1.2]. One agent owns it, and it carries a large table-driven test suite:

- ints are accepted for floats;
- sequences of any kind are accepted for arrays, with length checked against Blender's message `sequences of dimension 0 should contain 3 items, not 2`;
- enum strings are checked with `enum "FOO" not found in ('A', 'B')`;
- enum-flag sets accept a set, list or tuple;
- floats and ints are clamped to the hard range, with an INFO report when a clamp happens, since Blender clamps silently;
- strings are truncated to `maxlen` on a UTF-8 boundary;
- `Math` constructors are accepted wherever the shape matches;
- for pointers, the `fixed_type` is checked.

### 9.3 Paths

```rust
// crates/frap-rna/src/ptr.rs   // CONTRACT
pub struct RnaPtr { pub root: Root, pub path: SmallVec<[Seg; 6]>, pub srna: &'static StructDef }
pub enum Root { Data /*bpy.data*/, Context /*bpy.context*/, Id(IdAny) }
pub enum Seg { Prop(&'static str), Index(usize), Key(SmolStr) }   // .name, [3], ['Cube']
impl RnaPtr {
    pub fn repr(&self, names: &dyn IdNames) -> String;            // bpy.data.objects['Cube'].modifiers["Bevel"]
    pub fn path_from_id(&self) -> Option<String>;                 // modifiers["Bevel"].width  (F-curve data_path form)
}
```

Path resolution (`frap-session/src/resolve.rs`) goes through these steps:

1. Parse the path into `Seg`s. The parser handles `bpy.data`, `bpy.context`, `D.`, `C.`, attribute access, `['name']`, `[i]` (negative allowed) and `[i]` on an array property (which yields a scalar).
2. Walk the segments read-only. Whenever the walk reaches an ID pointer (`Field::Id`), it **re-roots** at `Root::Id`. A path therefore never borrows across datablocks, which keeps the borrow checker trivial.
3. The result is an `RnaPtr`, plus an optional trailing property or array index.
4. Then one of the following runs: `get`, `set` (which walks again mutably from the owning ID, through `IdMap::get_mut`, so the change is logged), or `call`.

### 9.4 Traits implemented by data structs

```rust
// crates/frap-rna/src/traits.rs   // CONTRACT
pub enum Field<'a> { Value(Value), Struct(&'a dyn Rna), Id(Option<IdAny>), Coll(&'a dyn RnaColl) }

pub trait Rna: Any + Send + Sync {
    fn srna(&self) -> &'static StructDef;                       // refined (e.g. BevelModifier)
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError>;
}
pub trait RnaMut: Rna {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError>;
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<&mut dyn RnaMut, RnaError>;
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<&mut dyn RnaCollMut, RnaError>;
}
pub trait RnaColl {
    fn item_type(&self) -> &'static StructDef;
    fn len(&self) -> usize;
    fn key_of(&self, i: usize) -> Option<&str>;                 // name for ['..'] lookups and JSON listing
    fn find(&self, key: &str) -> Option<usize>;                 // node sockets: first *enabled* match
    fn item(&self, i: usize) -> Field<'_>;
}
pub trait RnaCollMut: RnaColl { fn item_mut(&mut self, i: usize) -> Result<&mut dyn RnaMut, RnaError>; }
pub enum SetOutcome { Done, Clamped(Value), NeedsHook }         // NeedsHook => §9.5
```

Most bindings are one line per property, written with the `rna_struct!` macro. Plain structs need no hand-written binding at all, because the bindings are generated (§4.2).

```rust
// crates/frap-scene/src/id/object/rna.rs  (hand-written part of Object)
rna_struct! { Object as "Object" {
    name               => header.name,                      // string, unique_name via hook
    location           => location,                         // [f32;3]
    rotation_euler     => rotation_euler (tag = Euler(rotation_mode)),
    rotation_mode      => hook,                             // converts between modes (§9.5)
    scale              => scale,
    data               => id(data),
    parent             => hook,                             // matrix_parent_inverse semantics
    modifiers          => coll(modifiers, srna = "ObjectModifiers"),
    vertex_groups      => coll(vertex_groups, srna = "VertexGroups"),
    material_slots     => coll(material_slots),
    matrix_world       => hook,                             // evaluated read; decomposing write
    dimensions         => hook,                             // evaluated bounds
    type               => readonly(object_type()),
    children           => python_only hook,
}}
```

### 9.5 Hooks and functions (global tier)

Anything that needs more than its own struct is registered globally, one file per hook. That includes RNA functions (`new`, `remove`, `link`, `keyframe_insert`, `select_set`, `frame_set`, `from_pydata`, `vertex_group.add`, `nodes.new`, `links.new`, `ray_cast`, `path_resolve`…) and setters with side effects (`rotation_mode` conversion, `parent`, `name` uniqueness, `light.type` re-typing, `dimensions`, `matrix_world`, `frame_current`).

```rust
// crates/frap-scene/src/host.rs   // CONTRACT
pub trait Host {                                    // implemented by frap-session::Session
    fn main(&self) -> &Main;
    fn main_mut(&mut self) -> (&mut Main, &mut ChangeLog);
    fn edit(&mut self) -> &mut EditState;
    fn win(&mut self) -> &mut WindowState;
    fn ctx(&self) -> Context<'_>;
    fn eval(&mut self) -> (&dyn HostRead, &mut dyn Evaluator); // split borrow helper
    fn report(&mut self, r: Report);
    fn flush_edit(&mut self, mesh: IdRef<Mesh>);
    fn set_path(&mut self, ptr: &RnaPtr, prop: &str, v: Input) -> Result<(), RnaError>; // used by anim
}
pub type RnaFuncImpl = fn(&mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError>;
pub type RnaSetHook  = fn(&mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError>;
pub type RnaGetHook  = fn(&dyn HostRead, this: &RnaPtr, prop: &'static PropDef) -> Result<Value, RnaError>;

pub struct RnaFuncReg { pub srna: &'static str, pub func: &'static str, pub imp: RnaFuncImpl }
#[linkme::distributed_slice] pub static RNA_FUNCS: [RnaFuncReg];
#[linkme::distributed_slice] pub static RNA_HOOKS: [RnaHookReg];   // set and/or get per (srna, prop)

// crates/frap-op-anim/src/funcs/keyframe_insert.rs — one agent, one file
#[rna_func("bpy_struct", "keyframe_insert")]       // matched on every struct (inherited)
fn keyframe_insert(h: &mut dyn Host, this: &RnaPtr, a: &mut FuncArgs) -> Result<Value, RnaError> {
    let data_path: SmolStr = a.positional_or("data_path")?;       // keyword-only after it (5.2)
    let index: i64 = a.kw_or("index", -1)?;
    let frame: Option<f32> = a.kw_opt("frame")?;
    /* resolve owning ID, ensure Action "<Name>Action" + slot "OB<Name>" + layer/strip/channelbag,
       group "Object Transforms", insert BEZIER/AUTO_CLAMPED keys, recalc handles */
    Ok(Value::Bool(true))
}
```

`FuncArgs` binds positional and keyword arguments against `FuncDef.params` and enforces positional-only and keyword-only markers. For example, `obj.keyframe_insert("location", 2, 20)` raises Blender's TypeError. It also coerces each argument with `coerce()`.

### 9.6 Python-only members and 4.x compatibility errors

- **Python-only members** such as `children`, `users_collection`, `from_pydata`, `shade_smooth` and `shade_flat` come from `schema/overrides.toml`. They are flagged `PYTHON_ONLY` and implemented as hooks.
- **4.x compatibility errors** are listed in `frap-rna/src/compat.rs` as `(srna, member) → message`, for example `("Action","fcurves") → "Action.fcurves was removed in Blender 5.0; use action.layers[0].strips[0].channelbag(slot).fcurves"`. They are reported as `AttributeError` carrying a `hint`.

---

## 10. Operator registry (`frap-op`, `frap-macros`, `frap-spec`)

### 10.1 Definitions

```rust
// crates/frap-op/src/def.rs   // CONTRACT
pub struct OperatorDef {
    pub spec: &'static OperatorSpec,        // generated: idname, bl_idname, label, description, bl_options, props
    pub poll: fn(&Context) -> Result<(), PollFail>,
    pub exec: fn(&mut OpCtx, &OpArgs) -> OpResult,   // the macro wraps the typed fn
    pub kind: OpKind,                       // Normal | Macro { steps: &'static [&'static str] }
    pub dyn_enums: &'static [(&'static str, DynEnumFn)],
    pub status: Support,                    // Full | Partial(&'static str) | HeadlessDeviation(&'static str)
}
#[linkme::distributed_slice] pub static OPERATORS: [OperatorDef];

pub struct Registry { by_idname: BTreeMap<&'static str, &'static OperatorDef>, known: &'static [OperatorSpec] }
impl Registry {
    pub fn global() -> &'static Registry;  // built once from OPERATORS + frap_spec::ALL_OPS
    pub fn lookup(&self, idname: &str) -> Lookup;  // Implemented(def) | KnownNotImplemented(spec) | Unknown{suggestions}
}

pub type DynEnumFn = fn(&Context) -> Vec<EnumItemOwned>;          // mode_set.mode, modifier_add.type, ...
```

### 10.2 Implementing an operator (the whole unit of work)

```rust
// crates/frap-op-object/src/ops/shade_smooth_by_angle.rs
use frap_op::prelude::*;
use frap_spec::ops::object::ShadeSmoothByAngle as P;

#[operator(poll = poll::objectmode_with_selected_meshes)]   // idname taken from P::IDNAME
fn exec(cx: &mut OpCtx, p: P) -> OpResult {
    for ob in cx.ctx().selected_editable_objects() {
        let Some(me) = cx.mesh_of(ob) else { continue };
        let mesh = cx.mesh_mut(me)?;                      // COW + change log
        frap_mesh::shading::set_sharp_from_angle(mesh, p.angle.value, p.keep_sharp_edges.value);
        frap_mesh::shading::remove_sharp_face(mesh);
    }
    Ok(OpStatus::Finished)
}
```

`#[operator]` does five things:

1. It checks at compile time that the function takes the generated props type.
2. It emits `OperatorDef` into `OPERATORS`.
3. It wraps `exec` with `P::from_args`.
4. It attaches `status` and `dyn_enums` if they are given.
5. It adds a `#[test]` that the op's spec exists in `frap_spec` (catching an idname typo).

### 10.3 Invocation pipeline (identical for every front-end)

```
Session::invoke(idname, Inputs{positional: [], kwargs: IndexMap<String, Input>})
 1. Registry::lookup  -> Unknown => TypeError-like "bpy.ops.mesh.bevl not found" + did-you-mean
                      -> KnownNotImplemented => NOT_IMPLEMENTED error with Blender signature in payload
 2. positional args   -> TypeError (Blender ops are keyword-only)
 3. per kwarg: find PropDef (unknown => 'keyword "sizee" unrecognized' + suggestions),
    macro sub-op kwargs (TRANSFORM_OT_translate={...} or flattened TRANSFORM_OT_translate.value=...)
    -> nested OpArgs, coerce() each value; dynamic enums validated against DynEnumFn(ctx)
 4. OpArgs { values: IndexMap<&'static str, Value>, is_set: BitSet, nested: IndexMap<&'static str, OpArgs> }
    (defaults filled for unset props; SKIP_SAVE irrelevant: frap never remembers last-used values)
 5. poll(ctx)         -> RuntimeError "Operator bpy.ops.X.poll() <msg>"
 6. exec inside the command transaction (§12); Macro kind runs steps in order with nested OpArgs
 7. OpStatus::Finished -> commit;  Cancelled -> rollback, status CANCELLED + reports; Err -> rollback, ERROR
```

```rust
pub struct OpCtx<'h> { host: &'h mut dyn Host, reports: Vec<Report>, result: IndexMap<SmolStr, Value>, op: &'static OperatorSpec }
impl OpCtx<'_> {
    pub fn ctx(&self) -> Context<'_>;
    pub fn main(&self) -> &Main;
    pub fn object_mut(&mut self, r: IdRef<Object>) -> Result<&mut Object, OpError>;
    pub fn mesh_mut(&mut self, r: IdRef<Mesh>) -> Result<&mut Mesh, OpError>;
    pub fn bmesh_mut(&mut self, me: IdRef<Mesh>) -> Result<&mut BMesh, OpError>; // edit state, COW
    pub fn add_object(&mut self, name: &str, data: Option<IdAny>, at: Placement) -> IdRef<Object>; // links into ctx.collection, selects, activates
    pub fn evaluated(&mut self, ob: IdRef<Object>) -> Result<Arc<EvaluatedObject>, OpError>;
    pub fn report(&mut self, level: ReportLevel, msg: impl Into<String>);
    pub fn result(&mut self, key: &str, v: impl Into<Value>);   // op-specific payload (§13.3)
    pub fn run_op(&mut self, idname: &str, args: OpArgs) -> OpResult; // ops calling ops (duplicate_move etc.)
}
pub enum OpStatus { Finished, Cancelled }
pub enum OpError { Runtime(String) /* "Error: Cannot render, no camera" */, Rna(RnaError), Poll(PollFail),
                   NotImplemented(String), Io(String), Gpu(String) }
```

### 10.4 "Is set" semantics and shared helpers

`Prop<T> { value: T, is_set: bool }`. Shared helpers in `frap-op/src/helpers/` implement Blender's common patterns once:

- `placement(cx, &location, &rotation, &align, &scale)` uses the cursor when location is unset, derives rotation from `align` when it is unset, and treats the scale sentinel `(0,0,0)` as unset [ops-object §1.3];
- `add_primitive_or_merge(cx, name, mesh, placement, enter_editmode)` creates an object in object mode, or merges into the edit mesh and selects only the new geometry when in edit mode;
- `frame_range_or_scene(cx, &frame_start, &frame_end)`;
- `iter_edit_meshes(cx)`, the multi-object edit loop.

---

## 11. Front-ends: IR, value grammar, bpy-line and short form (`frap-lang`)

### 11.1 IR (the boundary every front-end lowers to)

```rust
// crates/frap-lang/src/ir.rs   // CONTRACT
pub struct Stmt { pub kind: StmtKind, pub span: Span, pub src: Arc<str>, pub line: u32 }
pub enum StmtKind {
    Expr(Expr),                                 // bare expression: get, or call (op / RNA func)
    Assign { target: Expr, op: AssignOp, value: Expr },   // =  +=  -=  *=  /=
    Print(Vec<Expr>),                           // print(a, b) -> JSON values
    Import,                                     // `import bpy`, `import math`, `from mathutils import ...`: no-op
    Pass,                                       // blank / comment-only
}
pub enum Expr {
    Lit(Lit, Span), Name(SmolStr, Span),        // names: bpy, D, C, math, mathutils ctors, radians, True...
    Attr(Box<Expr>, SmolStr, Span), Index(Box<Expr>, Box<Expr>, Span),
    Call { callee: Box<Expr>, args: Vec<Expr>, kwargs: Vec<(SmolStr, Expr)>, span: Span },
    Tuple(Vec<Expr>, Span), List(Vec<Expr>, Span), Set(Vec<Expr>, Span), Dict(Vec<(Expr, Expr)>, Span),
    Unary(UnOp, Box<Expr>, Span), Binary(BinOp, Box<Expr>, Box<Expr>, Span),
}
pub enum Lit { Int(i64), Float(f64), Str(String), Bool(bool), None }
```

The executor (`frap-session/src/exec.rs`) evaluates an `Expr` to an `Input` or a pointer:

- arithmetic is folded;
- `math.pi`, `math.radians`, `math.degrees`, `math.sqrt`, `math.sin`, `math.cos` and the bare `radians`/`pi` are supported;
- `Vector`, `Euler`, `Quaternion`, `Matrix` and `Color` become `Input::Math`, and their `.to_tuple()` and `@` matrix products cover the common cases;
- a path expression on the right-hand side is a **read**, for example `obj.location = bpy.data.objects['A'].location`;
- calls are dispatched by callee shape:
  - `bpy.ops.<ns>.<op>(...)` goes to `invoke`;
  - `<path>.<func>(...)` goes to an RNA function;
  - anything else is a NameError explaining what frap supports.

Nested calls are evaluated innermost first, for example `mesh.materials.append(bpy.data.materials.new("Red"))`. This removes most of the need for variables. **Assigning to a bare name** (`cube = ...`) produces `frap.UnsupportedSyntax` with a hint that shows the equivalent path form, for example `bpy.data.objects['Cube']`. Loops, `def` and `if` give the same error.

### 11.2 Lexical and value grammar (shared)

```
line         := stmt? comment? NEWLINE   ; bracket depth > 0 continues the line (Python implicit joining)
comment      := '#' .*
int          := ['-'] digit+ | '0x' hex+
float        := ['-'] digit* '.' digit* exp? | digit+ exp | 'inf' | 'nan' (rejected with error)
string       := '...' | "..." with \\ \' \" \n \t \uXXXX escapes ; r'...' raw ; no f-strings
bool/none    := True | False | None          (short form also accepts true/false/none)
tuple        := '(' ')' | '(' e ',' ')' | '(' e (',' e)+ [','] ')'
list / set / dict := '[' ... ']' | '{' e, ... '}' | '{' k ':' v, ... '}'   ; set() empty set
```

### 11.3 bpy-line form

A Python statement subset:

- `bpy.ops.mesh.primitive_cube_add(size=2, location=(0,0,1))`
- `bpy.data.objects['Cube'].location = (1,2,3)`
- `bpy.context.object.modifiers['Bevel'].width = 0.1`
- `bpy.context.object.location.x += 1`
- `bpy.data.materials['M'].node_tree.links.new(nodes['Image Texture'].outputs['Color'], ...)`. Inside a single expression, the relative `nodes[...]` is **not** allowed; paths must be absolute.
- `print(bpy.context.selected_objects)`
- a bare `bpy.data.objects['Cube'].dimensions`, which reads the value

`D` and `C` are accepted as aliases for `bpy.data` and `bpy.context`, as in the Blender console.

### 11.4 Short form

```
stmt      := op_call | assign | get
op_call   := opname (WS kwarg)*                  ; opname = ns '.' ident, ns ∈ registry namespaces
kwarg     := key '=' sval
key       := ident ('.' ident)?                  ; TRANSFORM_OT_translate.value=0,0,1 (macro sub-op)
sval      := scalar (',' scalar)*                ; comma list -> tuple: location=0,0,1
           | '{' ident (',' ident)* '}'          ; enum set: type={VERT,EDGE}
           | '(' ... ')' | '[' ... ']' | quoted string
scalar    := number | bareword | quoted string | true | false | none
             ; bareword -> Str; coerce() turns it into Enum/String as the PropDef says
assign    := path WS? '=' WS? sval                ; objects['Cube'].location = 1,2,3
get       := path                                 ; path may start with D., C., bpy., or a bpy.data collection name
```

Disambiguation:

- A line whose first token is `bpy`, `D`, `C`, `print`, `import` or `from`, or which contains `(` directly after the first dotted name, is parsed as bpy-line.
- Otherwise, if its first dotted name is a registered operator namespace (`mesh`, `object`, `frap`, …), it is a short-form op call.
- Otherwise it is a short-form assign or get.

The two forms may be mixed freely in one stream.

The front-end decides the form per line. A test corpus (`crates/frap-lang/tests/corpus/*.txt`) pairs each input with its expected IR in `insta` snapshots. Agents can grow the corpus without conflicts because every case is its own file.

### 11.5 Future front-ends

`frap-session` exposes the same four entry points that the IR executor uses:

```rust
impl Session {
    pub fn invoke(&mut self, idname: &str, kwargs: IndexMap<String, Input>) -> Response;
    pub fn get(&mut self, path: &str /* or &RnaPtr */) -> Response;
    pub fn set(&mut self, path: &str, value: Input) -> Response;
    pub fn call(&mut self, path: &str, func: &str, args: Vec<Input>, kwargs: IndexMap<String, Input>) -> Response;
}
```

A future `frap-lua` crate exposes `bpy.ops` and `bpy.data` userdata whose `__index`/`__newindex`/`__call` map onto these, using `mlua` with `serde` for `Input`. Nothing below `frap-session` changes.

---
## 12. Session, transactions, execution (`frap-session`)

```rust
pub struct Session {
    state: State,                       // everything that is rolled back
    eval: EvalCache,                    // keyed by gen; safe across rollback (§5.3)
    render: Option<Box<dyn RenderService>>, // lazily created GPU device; never snapshotted
    opts: SessionOptions,               // continue_on_error, response verbosity, max_items, cwd
    line: u32,
}
#[derive(Clone)] struct State { main: Main, win: WindowState, edit: EditState, prefs: Preferences }
// Clone is cheap: IdMaps of Arcs, BMeshes in Arcs.

impl Session {
    pub fn new(startup: Startup) -> Self;                 // Factory | Empty | File(path)
    pub fn exec_line(&mut self, src: &str) -> Vec<Response>;  // one Response per statement
    fn transaction<R>(&mut self, f: impl FnOnce(&mut Self) -> Result<R, FrapError>) -> Result<R, FrapError> {
        let snapshot = self.state.clone();                // O(#IDs)
        let r = std::panic::catch_unwind(AssertUnwindSafe(|| f(self)))
            .unwrap_or_else(|p| Err(FrapError::internal_panic(p)));
        if r.is_err() || self.cancelled() { self.state = snapshot; }
        r
    }
}
```

- **Granularity: one statement is one transaction.** That covers an op call, an assignment, a function call, and a nested-call expression taken as a whole.
- A read (get or print) runs in a transaction as well, because reading can flush edit data. Its change log is dropped.
- A panic anywhere is caught. The command is rolled back and reported as `INTERNAL`, and the stream continues. Release builds use `panic = "unwind"`.
- `ed.undo` and `ed.redo` come almost for free: keep the last N committed `State`s (P2, M8).
- External side effects such as writing a file, a render or an export cannot be rolled back. They run last inside the op, after every fallible step. Files are written to a temporary path in the same directory and renamed on success.

---

## 13. JSON response protocol

### 13.1 Stream contract

- **stdin:** UTF-8 text, statements separated by newlines (bracket continuation allowed). `frap run file.py` and `frap -c "<line>"` read from those sources instead.
- **stdout:** exactly one JSON object per statement (JSON Lines), in order. A final `{"event":"end","ok":true,"stats":{...}}` is written at EOF unless `--no-end-event` is given.
- **Exit code:** 0 normally. 1 if `--stop-on-error` was given and an error occurred. 2 on a CLI usage error.

### 13.2 Shapes

```jsonc
// success: operator
{"seq":3,"line":3,"kind":"op","op":"mesh.primitive_cube_add","ok":true,"status":"FINISHED",
 "reports":[],
 "result":{"object":{"$ref":"bpy.data.objects['Cube.001']","type":"MESH"}},
 "changes":{"added":["bpy.data.objects['Cube.001']","bpy.data.meshes['Cube.001']"],"removed":[],
            "modified":["bpy.data.collections['Collection']","bpy.data.scenes['Scene']"]},
 "context":{"mode":"OBJECT","active_object":"Cube.001","selected_objects":["Cube.001"]}}

// success: get
{"seq":4,"line":4,"kind":"get","ok":true,"path":"bpy.data.objects['Cube'].location",
 "value":[1,2,3],"type":{"rna":"FLOAT","array":[3],"subtype":"TRANSLATION","math":"Vector"}}

// success: set (value echoes the stored value, after coercion/clamping)
{"seq":5,"line":5,"kind":"set","ok":true,"path":"bpy.data.objects['Cube'].modifiers[\"Bevel\"].segments",
 "value":1000,"reports":[{"type":"INFO","message":"value 5000 clamped to hard max 1000"}],
 "changes":{"modified":["bpy.data.objects['Cube']"]}}

// success: RNA function call
{"seq":6,"line":6,"kind":"call","ok":true,"func":"bpy.data.materials.new",
 "value":{"$ref":"bpy.data.materials['Red']","type":"Material"},"changes":{"added":["bpy.data.materials['Red']"]}}

// CANCELLED (state unchanged)
{"seq":7,"line":7,"kind":"op","op":"object.join","ok":false,"status":"CANCELLED",
 "reports":[{"type":"WARNING","message":"Active object is not a selected mesh"}]}

// ERROR (state unchanged)
{"seq":8,"line":8,"kind":"op","op":"mesh.bevel","ok":false,"status":"ERROR",
 "error":{"exception":"TypeError","code":"UNKNOWN_KEYWORD",
          "message":"Converting py args to operator properties: : keyword \"offest\" unrecognized",
          "span":[16,22],"suggestions":["offset","offset_pct","offset_type"],
          "hint":"mesh.bevel default offset=0 is a no-op; pass offset=..."}}
```

### 13.3 What each kind of command reports

| Field | When | Content |
|---|---|---|
| `status` | op | `FINISHED`, `CANCELLED` or `ERROR`. Get, set and call use `FINISHED` or `ERROR`. |
| `reports` | always | Blender-style `{type: INFO\|WARNING\|ERROR\|OPERATOR\|PROPERTY, message}`. frap synthesises a message where Blender is silent (for example `modifier_remove` with an unknown name). |
| `result` | op | Op-specific named values set with `cx.result(...)`. Conventions: created objects go in `object`/`objects`; `mesh.remove_doubles` gives `{"removed": N}`; `render.render` gives `{"filepath": "...", "frames": [...]}`; `export_scene.gltf` gives `{"filepath", "bytes", "extensions_used"}`; `uv.*` gives `{"islands": N}`; select ops give `{"selected": {...counts}}`. |
| `changes` | any mutation | `added`, `removed` and `modified` ID refs, derived automatically from `ChangeLog`. |
| `context` | when mode, active or selection changed | `mode`, `active_object`, `selected_objects`; plus `edit_object` and `select_mode` in edit mode. |
| `edit_mesh` | edit-mode ops | Per object in mode: `{"Cube": {"verts":12,"edges":20,"faces":10,"tris":20,"selected":{"verts":4,"edges":4,"faces":1},"active":{"face":6}}}`. LLMs need these counts [ops-mesh §5.6]. |
| `value` / `type` | get, call | JSON value plus RNA type tag. Pointers become `{"$ref": repr}`. Collections become a list of `{"$ref"}` (or names with `--names-only`), truncated at `--max-items` (default 200) with `"truncated": true`. |
| `warnings_frap` | when frap deviates | For example `"frap evaluated mesh.loopcut headlessly from edge_index; Blender requires a 3D View"`. |

JSON value encoding:

| RNA type | JSON |
|---|---|
| f32 values | shortest round-trip f32 |
| NaN / Inf | written as JSON `null`, with a WARNING report (they can never be set) |
| vectors, Euler, colors | plain arrays; the math type is in `type.math` |
| quaternion | `[w,x,y,z]` |
| axis-angle | `[angle,x,y,z]` |
| matrix | row-major array of rows |
| enum-flag | sorted string list |

The envelope is described with `schemars` (`frap --describe --protocol-schema`).

---

## 14. Error handling

```rust
// crates/frap-rna/src/error.rs + frap-session/src/error.rs
pub struct FrapError {
    pub exception: PyException,          // TypeError | ValueError | KeyError | AttributeError | IndexError
                                         // | RuntimeError | NameError | SyntaxError | NotImplementedError
    pub code: Cow<'static, str>,         // stable machine code, see table
    pub message: String,                 // Blender wording where Blender has one
    pub span: Option<Span>, pub suggestions: Vec<SmolStr>, pub hint: Option<String>,
    pub detail: Option<serde_json::Value>, // e.g. the Blender signature for NOT_IMPLEMENTED
}
```

| Code | Exception | Source | Example message |
|---|---|---|---|
| `SYNTAX` | SyntaxError | frap-lang | `invalid syntax` with span |
| `UNSUPPORTED_SYNTAX` | SyntaxError | frap-lang | `frap accepts single statements; variables are not supported (use bpy.data.objects['Cube'])` |
| `NAME` | NameError | executor | `name 'cube' is not defined` |
| `UNKNOWN_OPERATOR` | AttributeError | registry | `Calling operator "bpy.ops.mesh.bevl" error, could not be found` |
| `NOT_IMPLEMENTED` | NotImplementedError | registry, hooks, modifiers | `mesh.knife_project exists in Blender 5.2 but is not implemented by frap` |
| `UNKNOWN_KEYWORD` / `BAD_ARG_TYPE` / `POSITIONAL_ARG` | TypeError | invoke, FuncArgs | Blender wording |
| `ENUM_NOT_FOUND` | TypeError | coerce | `enum "FOO" not found in ('A', 'B')` |
| `BAD_SEQUENCE_LEN` | ValueError | coerce | `sequences of dimension 0 should contain 3 items, not 2` |
| `KEY_NOT_FOUND` | KeyError | resolver | `bpy_prop_collection[key]: key "Nope" not found` |
| `INDEX_OUT_OF_RANGE` | IndexError | resolver | `bpy_prop_collection[index]: index 9 out of range, size 3` |
| `NO_ATTRIBUTE` / `READ_ONLY` | AttributeError | resolver | `bpy_struct: attribute "type" from "Object" is read-only` |
| `REMOVED_IN_5X` | AttributeError | compat table | names the 5.x replacement |
| `POLL_FAILED` | RuntimeError | invoke | `Operator bpy.ops.object.modifier_apply.poll() failed, context is incorrect` |
| `OP_ERROR` | RuntimeError | exec | `Error: Cannot render, no camera` |
| `IO` | RuntimeError | io ops | path and OS error |
| `GPU` | RuntimeError | render | no adapter, device lost, out of memory |
| `INTERNAL` | RuntimeError | panic | includes the panic message; please report |

- Library crates use `thiserror` enums that convert `Into<FrapError>`. Only `frap-cli` uses `anyhow`.
- Codes are `Cow<'static, str>`. Crates add their own codes as constants **in their own files**, so there is no central enum to fight over.

---

## 15. Schema export (`frap-schema`)

`frap --describe [--format manifest|jsonschema|tools-anthropic|tools-openai] [--filter 'mesh.*'] [--include-unimplemented]` writes JSON to stdout. The same data is available in-stream through `bpy.ops.frap.describe(target='mesh.bevel')` / `frap.describe target=Object.location`.

- **manifest** (default) is Blender-RNA-shaped. It contains:
  - `operators[]`: `idname`, `bl_idname`, `description`, `bl_options`, `poll` (a human description plus the required mode), `props[]` with `identifier`, `type`, `subtype`, `unit`, `array`, `default`, `hard_min`/`max`, `soft_min`/`max`, `enum_items`, `is_enum_flag`, `hidden`, `skip_save`, `dynamic_enum`, `unset_semantics` and `frap_note`, plus `frap_status`;
  - `structs[]`: props, funcs and Python-only members;
  - `context_members[]`, `modifier_types[]`, `node_types[]`;
  - `defaults_that_surprise[]`, curated from the research: export_apply, light_add 10 W, bevel offset=0, and so on.
- **jsonschema** gives one flattened JSON Schema (draft 2020-12) per operator, with no `$ref`. Enums become `enum`, enum sets become `array` + `uniqueItems`, and arrays get `minItems` = `maxItems`. Defaults are included.
- **tools-*:** exporting hundreds of operators as separate tools overwhelms agents, so frap emits a **small fixed toolset** instead:
  - `frap_exec(lines: string)`;
  - `frap_describe(target: string)`;
  - optionally, per-namespace tools with an `op` enum and a `oneOf` over the per-op schemas.

  This is documented in `docs/agents.md`.

Snapshot tests (`insta`) pin the manifest of each namespace in its own snapshot file, so namespaces don't conflict.

---

## 16. The `.frap` project format (`frap-project`)

```
scene.frap  (zip; entries in this exact order; all timestamps 1980-01-01 00:00:00)
├─ mimetype                      "application/vnd.frap+zip"   (Stored, first entry)
├─ manifest.json                 {"format":"frap","schema_version":1,"blender_rna_version":"5.2",
│                                 "generator":"frap 0.x","buffers":[...],"images":[...]}
├─ scene.json                    all datablocks, pretty-printed, keys in fixed order, Deflated
├─ buffers/<sha256-16>.bin       little-endian raw arrays (attributes, face offsets, shape keys, keyframes>N)
└─ images/<sha256-16>.<ext>      original encoded bytes (PNG/JPEG/WebP/EXR/HDR/KTX2), Stored
```

`scene.json` excerpt:

```jsonc
{"window":{"scene":"Scene","view_layer":"ViewLayer","cursor":...},
 "objects":{"Cube":{"type":"MESH","data":{"$id":"ME","name":"Cube"},"location":[0,0,0], "mode":"OBJECT",
                    "modifiers":[{"name":"Bevel","type":"BEVEL","width":0.1,...}], ...}},
 "meshes":{"Cube":{"face_offsets":{"$buf":"a1b2c3d4e5f60718","dtype":"i32","len":7},
                   "attributes":[{"name":"position","domain":"POINT","type":"FLOAT_VECTOR",
                                  "data":{"$buf":"…","dtype":"f32x3","len":8}}, ...]}},
 "materials":{"Material":{"node_tree":{"nodes":[...],"links":[...]}, ...}}, ...}
```

- **References** are written as `{"$id": code, "name": name}`. `IdKey`s are regenerated on load.
- **Buffers** are content-addressed (sha256, first 16 hex characters), so equal data is stored once, and diffs of `scene.json` show only *which* buffer changed. Arrays smaller than 64 elements are written inline, so small edits stay visible in diffs.
- **Serialization** uses serde derives on the data types plus `#[serde(with = "frap_project::buf")]` on `Arc<Vec<T>>` fields. These go through a scoped `BufferSink`/`BufferSource` context (thread-local during save and load), so the data crates do not depend on `frap-project`. Generated plain structs get serde derives automatically.
- **Saved state:** besides all datablocks, the file stores selection (bases), the active object, `Object.mode`, the cursor, tool settings and window state. Edit-mode BMeshes are flushed to `Mesh` before saving, and edit mode is entered again on load.
- **Orphans:** IDs with 0 users and no fake user are not written [ops-object §1.11]. The save op reports how many were dropped.
- **Versioning:** `schema_version` is an integer. `frap-project/src/migrate/v{N}_to_v{N+1}.rs` holds one migration per file, operating on `serde_json::Value` before typed deserialization. Unknown fields fail the load with a clear error, unless `--lenient`.
- **Operators:** `wm.save_as_mainfile(filepath, compress, relative_remap, copy, ...)`, `wm.save_mainfile`, `wm.open_mainfile(filepath, load_ui, use_scripts)`, `wm.read_homefile(use_empty)`, `wm.read_factory_settings`. Blender props that make no sense for frap are accepted and ignored with an INFO report. A `.blend` path gives the error `frap cannot write .blend; use a .frap path` (or "read", when opening).
- **Determinism:** fixed entry order, fixed timestamps, `serde_json` with `preserve_order`, shortest-f32 floats and no platform-dependent data. Saving the same state twice gives byte-identical zips. A test enforces this.

---

## 17. glTF / GLB (`frap-gltf`, `frap-gltf-blender`, codecs)

### 17.1 `frap-gltf`: format only, independently testable

```
crates/frap-gltf/src/
  json/{root,asset,scene,node,mesh,accessor,buffer,material,texture,image,sampler,camera,skin,animation}.rs
  ext/<extension_name>.rs        # one file per extension: typed struct + (de)serialize + validation
  glb.rs                         # container read/write (JSON 0x20 pad, BIN 0x00 pad, unknown chunks ignored)
  accessor.rs                    # encode/decode: normalized ints, sparse, byteStride, min/max, quantized types
  builder.rs                     # BufferBuilder: append views with alignment; interleave option (frap_*)
  validate.rs                    # structural validation (indices in range, required ext present)
```

```rust
pub struct Document {                     // all glTF top-level arrays, serde
    pub asset: Asset, pub scene: Option<u32>, pub scenes: Vec<Scene>, pub nodes: Vec<Node>, /* ... */
    pub extensions_used: Vec<String>, pub extensions_required: Vec<String>,
    pub extensions: Extensions, pub extras: Option<Box<RawValue>>,
}
pub struct Extensions(IndexMap<String, serde_json::Value>);   // lossless for unknown extensions
impl Extensions {
    pub fn get<E: Extension>(&self) -> Result<Option<E>, GltfError>;
    pub fn set<E: Extension>(&mut self, e: Option<E>);
}
pub trait Extension: Serialize + DeserializeOwned { const NAME: &'static str; const REQUIRED_BY_DEFAULT: bool = false; }
```

Typed extensions (one agent per batch of files): KHR_lights_punctual; KHR_materials_{emissive_strength, transmission, clearcoat, sheen, ior, specular, volume, iridescence, anisotropy, dispersion, unlit, variants, pbrSpecularGlossiness (read), diffuse_transmission}; KHR_texture_transform; KHR_mesh_quantization; KHR_draco_mesh_compression; EXT_meshopt_compression; KHR_meshopt_compression; EXT_texture_webp; KHR_texture_basisu; EXT_mesh_gpu_instancing; KHR_animation_pointer; KHR_node_visibility.

### 17.2 `frap-gltf-blender`: semantic mapping that mirrors io_scene_gltf2 5.2.41

The stages mirror the add-on's own modules [gltf §4.0], so porting and reviewing them can be split across agents:

```
crates/frap-gltf-blender/src/export/
  settings.rs          # generated ExportSettings (110 Blender props) + FrapExportExt (frap_* props)
  vtree.rs             # virtual tree: roots sorted natural/case-insensitive, filters (selection, visible, collection...)
  rest_frame.rs        # frame_set(0) unless export_current_frame; restore after
  nodes.rs             # TRS per object (Y-up swizzle, identity snapping 2e-6, never node.matrix), post-order
  mesh/primitives.rs   # one primitive per used material slot; corner split key (§6.1 split.rs); looptris
  mesh/attributes.rs   # NORMAL (4 dp), TEXCOORD_n (1-v), COLOR_n incl. fake COLOR_0 rule, TANGENT, JOINTS/WEIGHTS
  mesh/morph.rs        # shape keys -> targets (POSITION/NORMAL deltas), sparse heuristic
  materials/*.rs       # one file per glTF material feature, consuming LoweredMaterial (§18)
  images.rs            # export_image_format AUTO/JPEG/WEBP/NONE, ORM packing, keep originals
  cameras.rs lights.rs # unit conversion (W -> cd / lux), spot cones, AREA warning
  skins.rs             # joints, IBMs, deform-bone filter
  animation/*.rs       # ACTIONS / ACTIVE_ACTIONS / BROADCAST / NLA_TRACKS / SCENE modes; sampling; CUBICSPLINE; pointer
  compress.rs          # Draco / meshopt / quantization dispatch to codec traits
  write.rs             # key order, empty pruning, extensionsRequired rules, GLB / .gltf / embedded
crates/frap-gltf-blender/src/import/
  settings.rs, nodes.rs, mesh.rs (merge vertices, shading detection), materials.rs (raise to node trees),
  skins.rs (bone_heuristic, guess_original_bind_pose), animation.rs, lights.rs, cameras.rs, decompress.rs
```

**Defaults are Blender's.** Without `frap_*` options the output follows Blender's semantics:

- non-interleaved views;
- `UNSIGNED_SHORT` or `UNSIGNED_INT` indices;
- `KHR_lights_punctual` and `KHR_texture_transform` marked required;
- the `fps_base` time quirk;
- frame 0 as the rest frame.

It is not byte-identical, which is a non-goal [gltf §7.2].

**frap-namespaced extras** on `export_scene.gltf`. They are spelled `frap_*` so they can never collide with Blender's names, and are listed in `--describe` under `frap_extension: true`:

| Property | Values / effect |
|---|---|
| `frap_texture_format` | `'AUTO'` or `'KTX2'` |
| `frap_basisu_mode` | `'ETC1S'` or `'UASTC'` |
| `frap_mesh_quantization` | bool |
| `frap_interleave` | bool |
| `frap_node_visibility` | bool |
| `frap_diffuse_transmission` | bool |
| `frap_compat` | `'BLENDER'` (default) or `'SPEC'`; `'SPEC'` drops the quirks |
| `frap_draco_backend` | which Draco backend to use |

**Behaviour of `export_use_gltfpack=True`:** frap performs the equivalent work in-process with `meshopt`, so the external gltfpack binary is never required. It adds an INFO report saying so.

### 17.3 Codec traits (`frap-codec-mesh`, `frap-codec-image`)

```rust
pub trait DracoCodec: Send + Sync {
    fn encode(&self, prim: &PrimitiveData, q: &DracoQuantization, level: u8) -> Result<Vec<u8>, CodecError>;
    fn decode(&self, bytes: &[u8], attr_map: &BTreeMap<String, u32>) -> Result<PrimitiveData, CodecError>;
}
pub trait MeshoptCodec: Send + Sync {
    fn encode_vertex(&self, data: &[u8], stride: usize, version: u8) -> Vec<u8>;
    fn encode_index(&self, idx: &[u32], mode: IndexMode) -> Vec<u8>;
    fn decode(&self, mode: MeshoptMode, filter: MeshoptFilter, count: usize, stride: usize, src: &[u8]) -> Result<Vec<u8>, CodecError>;
}
pub trait ImageCodec: Send + Sync {
    fn decode(&self, bytes: &[u8], mime: &str) -> Result<DecodedImage, CodecError>;
    fn encode(&self, img: &DecodedImage, fmt: EncodeFormat /* Png{bits}|Jpeg{q}|WebpLossy{q}|WebpLossless|Exr|Hdr|Ktx2Basis{..} */) -> Result<Vec<u8>, CodecError>;
}
```

---

## 18. Materials: one lowering shared by exporter and renderer (`frap-material`)

`frap-nodes` stores Blender node trees faithfully. The node data is laid out as follows:

- `Node{name, label, bl_idname, location, width, parent, mute, hide, props: PropBag, inputs, outputs}`;
- `Socket{name, identifier, ty, enabled, hide_value, default_value}`;
- `Link{from_node, from_socket(identifier), to_node, to_socket(identifier)}`.

RNA behaviour:

- `nodes.new(type)` takes a `bl_idname` and creates the socket tables from the generated `NodeTypeDef`.
- Name lookup on sockets returns the first *enabled* socket with that name; lookup by identifier fails, matching Blender [rna §0.10].
- `links.new(output, input)` replaces any existing link into that input.
- `update_sockets` hooks re-evaluate `enabled` when a mode property changes (Mix `data_type`, Math `operation`, Separate Color `mode`, Normal Map `convention`…).

Built-in node groups, `"glTF Material Output"` (Occlusion, Thickness, Dispersion, Iridescence…) and `"Smooth by Angle"`, live in `frap-nodes/src/builtin_groups/*.rs`.

```rust
// crates/frap-material/src/lib.rs   // CONTRACT
pub struct LoweredMaterial {
    pub base_color: Channel<[f32; 4]>, pub metallic: Channel<f32>, pub roughness: Channel<f32>,
    pub normal: Option<NormalMap>, pub occlusion: Option<TexRef>,
    pub emissive: Channel<[f32; 3]>, pub emissive_strength: f32,
    pub alpha_mode: AlphaMode,                      // Opaque | Mask{cutoff} | Blend  (from node patterns)
    pub double_sided: bool,                         // !use_backface_culling
    pub unlit: bool,
    pub ext: MaterialExts,                          // clearcoat, sheen, transmission, volume, ior, specular,
                                                    // iridescence, anisotropy, dispersion, diffuse_transmission
    pub vertex_color: Option<VertexColorUse>,       // which color attribute, how combined
    pub diagnostics: Vec<Report>,                   // unrecognised patterns => fallback + warning
}
pub struct Channel<T> { pub factor: T, pub texture: Option<TexRef> }
pub struct TexRef { pub image: IdRef<Image>, pub uv: UvSource /* Named | ActiveRender | Index */, pub channel: ChannelSel,
                    pub transform: Option<TexTransform>, pub sampler: Sampler, pub colorspace_is_data: bool }

pub fn lower(tree: &NodeTree, mat: &Material, images: &dyn ImageLookup) -> LoweredMaterial;      // node graph -> glTF-shaped
pub fn raise(lm: &LoweredMaterial, into: &mut Material, names: &mut NameAlloc);                    // importer: rebuild Blender-style graph
```

- `lower` is organised as one **pattern recogniser per file** (`patterns/base_color.rs`, `patterns/metallic_roughness.rs`, `patterns/normal_map.rs`, `patterns/alpha.rs`, `patterns/emission.rs`, `patterns/clearcoat.rs`, …), mirroring `io_scene_gltf2/blender/exp/material/*`.
- The **renderer consumes the same `LoweredMaterial`**. Whatever exports correctly also renders correctly, and the reverse holds too, so materials are tested once.
- **Round-trip test:** for every material fixture, check that `lower(raise(lower(x))) == lower(x)`.

---

## 19. Animation, armatures, shape keys

- **`frap-anim` (data plus pure evaluation, no scene dependency):**
  - `Action{slots, layers[{strips[{channelbags[{slot_handle, fcurves, groups}]}]}]}` and `FCurve{data_path, array_index, group, keyframe_points, extrapolation, modifiers(P2)}`;
  - `Keyframe{co, handle_left/right, interpolation, easing, handle types, back, amplitude, period, type}`;
  - `AnimData{action, action_slot_handle, nla_tracks, use_nla, action_blend_type, action_influence, action_extrapolation}`;
  - `fn evaluate_fcurve(&FCurve, frame: f32) -> f32`, which covers Bezier with Blender's handle calculation (`BKE_nurb_handle_calc` semantics in `handles.rs`), all 13 interpolations with their easings, and extrapolation;
  - `fn evaluate_anim(&AnimData, actions, frame) -> Vec<(data_path, index, value)>`, including NLA blending.
- **Applying animation:** `frap-eval/src/animate.rs` writes the evaluated channels into the original IDs through `Host::set_path` (RNA paths relative to the ID, the `path_from_id` form). It is called by `scene.frame_set`, by the `frame_current` setter hook, and per frame by `render(animation=True)` and the glTF exporter's sampler.
- **Armatures:**
  - `Armature.bones` hold the rest pose. The edit-mode `EditArmature{edit_bones}` lives in `EditState`.
  - `Object.pose.bones` holds the PoseBone channels; their `rotation_mode` defaults to `QUATERNION`.
  - `frap-eval/src/pose.rs` computes pose matrices.
  - The ARMATURE modifier does linear blend skinning, with dual quaternion as a P2. It reads `Mesh.deform_verts` by vertex-group name matched to bone names.
- **Shape keys:** the `Key` ID holds key blocks with absolute positions. Evaluation mixes them relatively. In edit mode, each key is a BMesh `ShapeLayer`, and the basis offset is propagated on write-back.
- **Ops:** `anim.keyframe_insert`, `anim.keyframe_insert_by_name`, `anim.keyframe_delete*`, `anim.keyframe_clear*`, `screen.frame_jump`, `screen.frame_offset`, `pose.*`, `armature.*`, and the RNA functions `keyframe_insert`/`keyframe_delete`, `fcurves.new`, `keyframe_points.insert`/`add`, `nla_tracks.new` and `strips.new`.
- **The keying-set and auto-group rules** (`Object Transforms`, the bone-name group, `CubeAction`/`OBCube` naming) live in `frap-op-anim/src/keying.rs`, which one agent owns.

---
## 20. Renderer (`frap-render-scene`, `frap-render`, `frap-color`)

### 20.1 Split for parallel development

```
frap-eval ──> frap-render-scene::build(&dyn HostRead, &mut Evaluator, &RenderRequest) -> RenderScene   (CPU, no wgpu)
                         │  plain data: meshes (corner-split, looptris), LoweredMaterials, images, lights, camera, world, settings
                         ▼
frap-render::Renderer { gpu: Gpu, engines: {Workbench, Eevee}, cache } ── Film (scene-linear f32 RGBA)
                         ▼
frap-color::display(&Film, &ViewSettings, &DisplaySettings) -> DisplayImage ──> frap-codec-image encode ──> file
```

```rust
// crates/frap-render-scene/src/lib.rs   // CONTRACT
pub struct RenderScene {
    pub resolution: [u32; 2],                       // resolution_x/y * percentage
    pub camera: RCamera,                            // view, proj (sensor_fit, shift, clip, ortho_scale), aspect
    pub objects: Vec<RObject>,                      // sorted by object name for determinism
    pub meshes: IndexMap<MeshKey, Arc<RMesh>>,      // MeshKey = evaluated fingerprint -> GPU cache key
    pub materials: IndexMap<MatKey, Arc<LoweredMaterial>>,
    pub images: IndexMap<ImageKey, Arc<DecodedImage>>,
    pub lights: Vec<RLight>,                        // Blender units preserved (W, W/m², spot_size, blend, radius)
    pub world: RWorld,                              // Color{rgb, strength} | Hdri{image, rotation, strength}
    pub settings: RSettings,                        // engine, eevee.taa_render_samples, use_shadows, film_transparent,
                                                    // display.shading (Workbench), view_settings, frame
}
pub struct RMesh { pub positions: Vec<[f32;3]>, pub normals: Vec<[f32;3]>, pub uvs: Vec<Vec<[f32;2]>>,
                   pub colors: Vec<Vec<[f32;4]>>, pub tangents: Option<Vec<[f32;4]>>,
                   pub indices: Vec<u32>, pub submeshes: Vec<(u32 /*slot*/, Range<u32>)>, pub edges: Vec<[u32;2]> }

// crates/frap-render/src/lib.rs   // CONTRACT
pub trait Engine { fn render(&mut self, gpu: &Gpu, cache: &mut GpuCache, scene: &RenderScene) -> Result<Film, RenderError>; }
pub struct Gpu { pub device: wgpu::Device, pub queue: wgpu::Queue, pub info: wgpu::AdapterInfo }
impl Gpu { pub fn new(opts: &GpuOptions) -> Result<Gpu, RenderError>; } // FRAP_WGPU_BACKEND, FRAP_ADAPTER, fallback adapter
pub struct Film { pub width: u32, pub height: u32, pub rgba: Vec<[f32; 4]> }   // scene-linear, straight alpha
```

### 20.2 Engines

| Engine | Blender equivalent | Implementation |
|---|---|---|
| `BLENDER_WORKBENCH` (M2) | Workbench render using `scene.display.shading` | `light`: STUDIO (frap-made studio rigs named like Blender's), MATCAP (frap-made matcaps), FLAT. `color_type`: MATERIAL (`diffuse_color`), OBJECT, RANDOM (hash of name, as in Blender), VERTEX, TEXTURE, SINGLE. Also `show_xray` with `xray_alpha`, wireframe via the `type` and `show_wireframes` options, `show_object_outline`, `show_shadows`, and `show_cavity` (P2). |
| `BLENDER_EEVEE` (M5) | EEVEE (also used for `CYCLES` with a warning) | Forward PBR (GGX + Lambert, multiscatter approximation, the Principled subset that `LoweredMaterial` carries). Point lights use cube shadow maps, spot lights 2D maps, sun lights 4 cascades. Light units are converted with the factors from EEVEE's light power functions, verified by golden renders of a diffuse plane. IBL uses an equirect HDRI, prefiltered into an irradiance SH and a GGX mip chain with a BRDF LUT asset. Transparency: BLENDED sorts per object back to front, DITHERED uses hashed alpha. Transmission samples a screen-space refraction of the opaque pass. Emission is supported. TAA accumulates `taa_render_samples` (default 64) jittered sub-frames, which also yields anti-aliasing and soft shadows. `film_transparent` is supported. |

- **Offscreen:** every target is a texture with `RENDER_ATTACHMENT | COPY_SRC` (`Rgba16Float` HDR accumulation) and is read back through a buffer whose rows are padded to 256 bytes.
- **Device lifetime:** the device is created lazily on the first render and reused for the rest of the session.
- **Caching:** `GpuCache` keeps GPU meshes keyed by `MeshKey`, textures by `ImageKey`, and pipelines by material feature bits. Animation renders reuse everything that is unchanged.

### 20.3 Color management (`frap-color`)

- **View transforms** (AgX, Filmic, Khronos PBR Neutral, ACES 1.3 and 2.0, False Color) are applied as **3D LUTs over a log2 shaper**, baked from Blender itself by `tools/blender/bake_view_luts.py`:
  1. Render a known HDR ramp and lattice EXR through each `view_transform` and `look` with `image.save_render`.
  2. Fit the shaper and LUT.
  3. Store the result in `assets/luts/*.cube` (generated, checked in).
- **Standard and Raw** are analytic.
- **`exposure` and `gamma`** are applied as Blender does: exposure before the transform, gamma after.
- **The view-transform list is fixed** and matches 5.2: `Standard`, `ACES 1.3`, `ACES 2.0`, `Khronos PBR Neutral`, `AgX`, `Filmic`, `Filmic Log`, `False Color`, `Raw`.
- **LUT application** is pure CPU (rayon over rows, deterministic). The same code is used in tests, so view-transform golden tests need no GPU.

### 20.4 Operators

- **`render.render`** supports `write_still` and `animation`, plus `frame_start` and `frame_end` with is-set semantics. It writes to `scene.render.filepath`, following Blender's rules: `//` relative to the .frap file or the cwd, the extension added when `use_file_extension`, `#` padding, and `0001` appended for animation [ops-object §4.17]. The result is also stored as `bpy.data.images['Render Result']`. Without a camera it fails with `Error: Cannot render, no camera`.
- **`render.opengl`** maps to Workbench rendering from the scene camera (P2).
- **`frap.render_contact_sheet`** is a frap extension, namespaced under `frap.`:

  ```
  bpy.ops.frap.render_contact_sheet(filepath='//sheet.png', views={'FRONT','RIGHT','TOP','PERSP'},
      engine='BLENDER_WORKBENCH', objects='VISIBLE'|'SELECTED', tile_size=(512,512), columns=0 /*auto*/,
      margin=0.05, labels=True, background=(0.2,0.2,0.2,1), use_scene_camera=False)
  ```

  - It builds N synthetic cameras that frame the bounding box of the chosen objects. Ortho views look along ±X, ±Y and ±Z, and `PERSP` matches the factory camera direction.
  - The tiles are rendered with the same engines and composited on the CPU. Labels use an embedded bitmap font from `assets/`.
  - It returns `{"filepath", "tiles": [{"view", "rect"}]}`.
  - The scene is never modified: the synthetic cameras exist only in `RenderScene`.

---

## 21. Golden test harness (`frap-golden`, `tools/blender`, `tests/golden`)

### 21.1 Case format

```python
# tests/golden/cases/mesh/extrude_region_move_top.py
# golden: startup=factory compare=objects,mesh:Cube order=exact tol=1e-5
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
#@frap  bpy.ops.frap.select_elements(type='FACE', indices=[5])
#@blender import bmesh; bm=bmesh.from_edit_mesh(bpy.context.object.data); bm.faces.ensure_lookup_table(); bm.faces[5].select_set(True); bmesh.update_edit_mesh(bpy.context.object.data)
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, 1)})
bpy.ops.object.mode_set(mode='OBJECT')
```

- **Header directives:**

  | Directive | Values |
  |---|---|
  | `startup` | `factory` or `empty` |
  | `compare` | `objects`, `mesh:<name>`, `materials`, `anim:<name>`, `glb:<path>`, `render:<path>` |
  | `order` | `exact` or `canonical` |
  | `tol` | numeric tolerance |
  | `expect_error` | the line number where an error is expected |
  | `skip_blender` | used only for frap-only features |

- **Line prefixes:** `#@frap` lines run only in frap, and `#@blender` lines run only in Blender. They cover the cases where the two must differ, such as headless-only operations, frap selection helpers, and the loopcut emulation [ops-mesh App. B.7].
- **In Blender,** `tools/blender/golden_runner.py` executes the statements one at a time inside `bpy.context.temp_override(window, area=VIEW_3D, region=WINDOW)`, as required by [ops-object §1.7]. It records the exception class for each line, then calls `dump_state.py`.
- **In frap,** the run is `frap run case.py --golden-dump out.json`. The runner uses the same line splitting and the same `#@` handling.

### 21.2 Shared dump schema (`GoldenState`, version 1)

```rust
pub struct GoldenState {
    pub lines: Vec<LineOutcome>,                 // ok | exception class (+ message, compared loosely)
    pub objects: BTreeMap<String, GObject>,      // type, parent, matrix_world, loc/rot/scale, modifiers, materials, mode
    pub meshes: BTreeMap<String, GMesh>,         // positions, edges, faces (corner verts), corner normals,
                                                 // uv layers (per corner), attributes, selection, sharp/seam
    pub evaluated: BTreeMap<String, GMesh>,      // with modifiers (M4+)
    pub materials: BTreeMap<String, GMaterial>,  // node tree as nodes/links/sockets defaults
    pub actions: BTreeMap<String, GAction>,      // slots, fcurves, keys; plus sampled values per frame
    pub context: GContext,                       // active, selected, mode, select_mode, cursor
    pub files: BTreeMap<String, GFile>,          // glb -> canonical glTF; png -> pixels+size
}
```

### 21.3 Comparators (`frap-golden/src/compare/*.rs`, one file each)

| Comparator | Method |
|---|---|
| `mesh_exact` | Index by index, for the ops on the order-stable list [ops-mesh §5.7]. |
| `mesh_canonical` | Vertices are matched by quantised position (1e-4 grid, with tie-break by neighbourhood). Faces become rotation-normalised position cycles and are compared as multisets. UVs are compared per (face-key, corner-position), and attributes per element key. |
| `gltf_canonical` | Parses both GLBs. Nodes are compared by name and hierarchy, meshes as multisets of triangles of full corner-attribute tuples, materials as normalised JSON with float tolerance, animations by sampled values, `extensionsUsed` as a set. `asset.generator` is ignored [gltf §7.2]. |
| `image_ssim` | SSIM ≥ threshold, plus a mean ΔE limit. Workbench is strict. EEVEE has a loose threshold against Blender plus a strict threshold against frap's own blessed output. |
| `numbers` | Relative/absolute tolerance, f32-aware. |

### 21.4 Fixtures and CI

- **Fixtures:** `tests/golden/fixtures/<area>/<case>.blender.json` (plus `.glb` and `.png`) are generated by `cargo xtask golden bless [--filter]`. This needs `$BLENDER_BIN`, then `PATH`, then `~/Applications/Blender-5.2.app/...`. The fixtures are checked in.
- **`cargo test -p frap-golden`** runs frap on every case and compares against the checked-in fixtures, so it works on any CI with no Blender installed.
- **Live mode:** `FRAP_GOLDEN_LIVE=1` reruns Blender. When Blender is missing, live tests are skipped with a visible notice, and fixture tests still run.
- **Recorded outcomes:** each fixture records the Blender version and build hash. A bless with a different Blender version causes a diff, which is the intended drift alarm.
- **Ownership:** every work package adds its own case files and fixtures, in its own area directory.

### 21.5 Other test layers

| Layer | Tool | Location |
|---|---|---|
| Unit | `cargo test` | colocated |
| Properties | `proptest` (BMesh invariants after random op sequences; coerce; naming; parser round-trips) | per crate `tests/prop_*.rs` |
| Snapshots | `insta` (IR, manifest, responses) | per crate |
| CLI transcripts | `trycmd`: stdin → expected JSONL, with redactions for paths | `tests/cli/<topic>/<name>.trycmd` |
| Render | `image-compare` SSIM; CI uses lavapipe (Linux) and WARP (Windows) | `tests/render/` |
| Fuzz (M8) | `cargo fuzz` on the lexer/parser and the GLB reader | `fuzz/` |

---

## 22. Determinism

1. **No observable `HashMap` order.**
   - Use `IndexMap` or `BTreeMap` everywhere data is serialised, iterated into output, or used to allocate names.
   - `rustc-hash` is allowed only for internal lookups.
2. **f32 storage everywhere Blender stores f32.** Use f64 only inside algorithms (QEM, LSCM, ABF, plane fits) and round back to f32.
3. **The RNG is Blender's `BLI_rng`** (`frap-math::rng`, a drand48-style LCG with Blender's seeding). It is used by `select_random`, `subdivide(fractal, seed)`, `randomize_transform` and the RANDOM Workbench colour, so results match Blender seed for seed. Nothing ever uses time-based or OS randomness.
4. **Parallelism must not change results.**
   - `rayon` may be used only where the reduction is order-independent or done in index order. Examples: per-row LUT application, per-island UV solves joined by island index.
   - Floating-point sums in parallel code are computed in fixed chunk order.
5. **Name allocation** depends only on the existing names and the request, never on the history of the IdKeys.
6. **Outputs are byte-stable.** `.frap` files, GLBs and JSON responses are byte-identical across runs and platforms for the same input. A CI job runs the CLI transcript suite on macOS, Linux and Windows and diffs the results.
7. **GPU output is the documented exception.** It may differ across vendors and drivers, which is why render goldens use tolerances. With the same adapter, frap's output is deterministic: fixed TAA jitter sequence (R2/Halton), no atomics-ordering dependence, fixed sort order for transparency.

---
## 23. Milestone plan and work packages

Conventions:

- **WP ownership is exclusive.** The listed paths are the only places that WP may edit. Two WPs never share a file.
- **"Needs"** names contracts only, never implementations. Every M0 contract exists as a compiling stub, so all WPs of a milestone can start on day 1. A few explicitly marked "soft" dependencies exist, where a WP works against fixtures until the other lands.
- **Every WP ships** its golden cases, CLI transcripts and `--describe` snapshot updates for what it adds. Each one exits only when its acceptance tests pass in CI.
- **Shared aggregation files are not edited by WPs.** These are `frap-ops-all/Cargo.toml`, the root `Cargo.toml`, and `schema/implemented.toml`, which uses one TOML file per crate under `schema/implemented/<crate>.toml` to avoid conflicts. The integrator updates them in the WP0.1 style, or WPs touch only their own entries.

### M0: Contracts and skeleton (shippable: `frap` reads a stream and answers every op with structured NOT_IMPLEMENTED or parse errors; `--describe --include-unimplemented` lists all 5.2 operators)

| WP | Title | Owns | Needs | Acceptance |
|---|---|---|---|---|
| 0.1 | Workspace, CI, integrator | root `Cargo.toml`, `rust-toolchain.toml`, `.github/`, `deny.toml`, `xtask/` (except `gen`), all `crates/*/Cargo.toml` at creation, `frap-ops-all/` | none | CI matrix (macOS arm64, Linux x86_64 with lavapipe, Windows with WARP) is green. A clean-container build with only cc/c++ passes. `cargo deny` passes. |
| 0.2 | Blender dumps and codegen | `tools/blender/*_dump.py`, `schema/`, `xtask/src/gen/`, `crates/frap-spec/`, every `*/src/gen/` | 0.3 types (`PropDef` etc.) | `cargo xtask gen --check` is clean. The spec drift test passes. Every dumped op has a typed props struct. Plain structs (Camera, Light, RenderSettings, all modifier settings) compile with `Default` equal to the fresh values. |
| 0.3 | Math, RNA core, naming | `crates/frap-math/`, `crates/frap-rna/` | none | `coerce()` table tests cover every row in [rna §1.2] with Blender messages. `unique_name` reproduces all verified cases. `BliRng` reproduces a reference sequence dumped from Blender (`select_random` output). Euler and matrix decomposition match Blender within 1e-6. |
| 0.4 | Operator framework and macros | `crates/frap-macros/`, `crates/frap-op/` | 0.3, 0.2 (soft) | A dummy op in a test crate registers through linkme+automod. Invoke-pipeline unit tests cover: unknown op, unknown keyword with suggestions, positional args rejected, macro nested args (both spellings), dynamic enum validation, `is_set`. |
| 0.5 | Front-ends | `crates/frap-lang/` | 0.3 (`Input` only) | A corpus of at least 300 lines (bpy-line and short form, every syntax in §11) has IR snapshots. Errors carry spans. Fuzzing for 10 minutes produces no panic. |
| 0.6 | Session and CLI skeleton | `crates/frap-session/`, `crates/frap-cli/`, `crates/frap-scene/src/{host,main,context}.rs` (stubs) | 0.3, 0.4, 0.5 | `echo '...' \| frap` produces JSONL with the protocol envelope. Transactions and rollback work against a stub `Main`. A panic in an op becomes `INTERNAL` and the stream continues. |
| 0.7 | Golden harness skeleton | `crates/frap-golden/`, `tools/blender/{golden_runner,dump_state}.py`, `tests/golden/` layout, `xtask golden` | 0.6 | Three cases have blessed Blender fixtures. The comparator compiles. The skip-if-no-Blender path is tested. |

Max parallelism: 7.

### M1: Object-mode modelling loop (shippable: primitives, object transforms, selection, get/set of any implemented property, RNA functions for data creation, .frap save/open, describe)

| WP | Title | Owns | Needs | Acceptance |
|---|---|---|---|---|
| 1.1 | Scene datablocks | `crates/frap-scene/src/{id,startup,naming_glue,users}.rs` and subdirs (except `id/object/ops_support`) | 0.3, 0.6 | `Main`/`IdMap` COW, rename, remove(do_unlink), users and orphans, factory and empty startup equal `startup.json`. RNA bindings for Object, Scene, ViewLayer, Collection, Camera, Light, World, Image (metadata only). Python-only members included. |
| 1.2 | Mesh and geometry basics | `crates/frap-mesh/` (except `primitives/`), `crates/frap-geom/` | 0.3 | Attributes and proxies, RNA for every element struct, `from_pydata`, corner, face and vertex normals with sharp edges and faces, looptris (quads and polyfill), `set_sharp_from_angle`, `shade_smooth`/`shade_flat` behaviour. |
| 1.3 | Session core | `crates/frap-session/` (resolver, executor, response, flush_edit stub) | 1.1, 1.2 (soft) | All path forms in [rna §1.1]. Get/set/call responses. `changes` and `context` deltas. Clamping reports. 4.x compat errors. |
| 1.4 | Primitives | `crates/frap-mesh/src/primitives/`, `crates/frap-op-mesh/src/ops/primitive_*.rs`, `crates/frap-op/src/helpers/placement.rs` | 0.4, 1.2 | All `mesh.primitive_*_add` match Blender **index-exact**: positions, edges, faces, UVs, defaults and is-set placement. Monkey depends on D2. |
| 1.5a | Object ops: selection, add, delete, duplicate, join, collections | `crates/frap-op-object/src/ops/{select_*,*_add,delete,duplicate*,join,collection_*,move_to_collection}.rs`, `crates/frap-op-data/src/funcs/{objects_new,collection_link,select_set,hide_set}.rs` | 0.4, 1.1 | Goldens for selection order, duplicate prefs (materials shared), join (CANCELLED reports), orphans after delete. |
| 1.5b | Object ops: transforms, parenting, origin, apply, shading, cursor | `crates/frap-op-object/src/ops/{transform_*,parent_*,origin_*,*_clear,shade_*,snap_*,mode_set}.rs`, `frap-scene/src/id/object/{matrix,parent}.rs` | 0.4, 1.1, 1.2 | `transform.translate/rotate/resize` in object mode with orient types and pivots, `parent_set` with inverse, `transform_apply`, `origin_set` (all types), shade ops, Euler conversion on `rotation_mode` set, and `matrix_world` write decomposition all match Blender. |
| 1.6 | .frap persistence | `crates/frap-project/`, `crates/frap-op-io/src/ops/wm_*.rs` | 1.1, 1.2 | Round trip is lossless (property test). Output is byte-identical across saves. Orphans dropped. Migration framework in place with a v0 to v1 test. |
| 1.7 | Schema export | `crates/frap-schema/`, `crates/frap-op-frap/src/ops/describe.rs`, `docs/agents.md` | 0.2, 0.4 | Manifest and JSON Schema snapshots. LLM tool formats validate against the provider schema subsets (a test in CI). |
| 1.8 | Golden M1 | `crates/frap-golden/src/compare/{mesh_exact,mesh_canonical,numbers}.rs`, `tests/golden/cases/object/**` | 0.7 | At least 60 object-mode cases blessed and green. |

Max parallelism: 9.

### M2: Static GLB round-trip and Workbench render (shippable: build, texture and export GLB matching Blender's exporter; import GLB; render a PNG)

| WP | Title | Owns | Needs | Acceptance |
|---|---|---|---|---|
| 2.1 | glTF format crate | `crates/frap-gltf/` | none | Reads all Khronos sample models (a vendored subset under `tests/data/gltf-samples/`, licences checked) and writes them back losslessly, modulo buffer layout. Unknown extensions round-trip. Validator-clean output (glTF-Validator optional in CI). |
| 2.2 | Nodes and material data | `crates/frap-nodes/`, `crates/frap-scene/src/id/{material,world,image,texture}.rs`, `crates/frap-op-data/src/{ops,funcs}/{material_*,nodes_*,links_*,images_*,worlds_*}.rs` | 0.2, 1.1 | `materials.new` gives Principled + Output at Blender locations. Socket lookup rules [rna §0.10]. `links.new` replaces existing links. Mix and Math dynamic sockets. Removed-node errors (SeparateRGB). |
| 2.3 | Material lowering (core) | `crates/frap-material/` (`patterns/{base_color,metallic_roughness,normal_map,emission,alpha,texture_transform,vertex_color,uv}.rs`, `raise/*`) | 2.2 | Pattern tests from the Blender exporter fixture materials. `lower(raise(lower(x))) == lower(x)`. |
| 2.4 | Image codecs | `crates/frap-codec-image/` | none | PNG 8/16, JPEG, WebP lossy and lossless, HDR, EXR decode and encode. Colorspace flags. Stored-bytes passthrough. |
| 2.5 | GLB export (static) | `crates/frap-gltf-blender/src/export/` (except `animation/`, `skins.rs`, `mesh/morph.rs`, `compress.rs`), `crates/frap-op-io/src/ops/export_scene_gltf.rs` | 2.1, 2.3, 2.4, 1.2 | `gltf_canonical` equal to Blender for at least 40 cases: hierarchy, multi-material, multiple UV sets, vertex colours (including the fake COLOR_0), flat, smooth and sharp normals, cameras and lights (with the flags), alpha modes, double-sided, texture transform, extras, `export_format` variants. |
| 2.6 | GLB import (static) | `crates/frap-gltf-blender/src/import/` (static parts), `crates/frap-op-io/src/ops/import_scene_gltf.rs` | 2.1, 2.3, 2.4 | Import → export round-trip equals the source GLB canonically. Same scene as Blender's import (`GoldenState` compare). |
| 2.7 | Workbench render | `crates/frap-render-scene/`, `crates/frap-render/src/{gpu,cache,workbench/**,readback}.rs`, `crates/frap-color/`, `tools/blender/bake_view_luts.py`, `assets/luts/`, `crates/frap-op-render/src/ops/render_render.rs` | 1.2, 2.3 (soft) | SSIM ≥ 0.97 against Blender Workbench SOLID/STUDIO on 10 scenes. AgX and Standard LUT goldens on CPU match within ΔE < 1. Output path rules. Headless on all three CI platforms. |
| 2.8 | Golden M2 | `crates/frap-golden/src/compare/{gltf_canonical,image_ssim}.rs`, `tests/golden/cases/gltf/**`, `tests/golden/cases/render/**` | 0.7 | Comparator self-tests (equal under permutation, different under a single-triangle change). |

Max parallelism: 8.

### M3: Edit mode, BMesh ops and UV (shippable: interactive-style modelling via edit-mode ops; UV unwrap and smart project)

| WP | Title | Owns | Needs | Acceptance |
|---|---|---|---|---|
| 3.1 | BMesh core | `crates/frap-bmesh/src/{types,pool,kernel,query,customdata,select,convert,validate}.rs` | 1.2 | Mempool LIFO order equals [ops-mesh B.6]. `from_mesh` → `to_mesh` is the identity on all primitives. proptest: 10k random kernel sequences keep validity. |
| 3.2 | Edit context and selection | `crates/frap-op-mesh/src/ops/{select_*,hide,reveal,loop_select,edgering_select}.rs`, `crates/frap-op-object/src/ops/mode_set_edit.rs`, `crates/frap-op-frap/src/ops/select_*.rs`, `crates/frap-session/src/edit_flush.rs` | 3.1 | Multi-object edit. Select-mode conversion table [ops-mesh §3.3]. `select_random` equals Blender for each seed. History and active element. |
| 3.3 | BMesh ops A | `crates/frap-bmesh/src/ops/{extrude_*,inset_*,delete,dissolve_*,duplicate,split,remove_doubles,collapse,pointmerge,contextual_create,triangle_fill,grid_fill,bridge_loops}.rs` and the matching `frap-op-mesh/src/ops/*.rs` | 3.1 | Golden counts and index-exact order for extrude, inset, split, merge and delete. Canonical comparison for the rest. |
| 3.4 | BMesh ops B | `crates/frap-bmesh/src/ops/{subdivide_edges,bevel/**,poke,triangulate,join_triangles,spin,solidify,wireframe,symmetrize,smooth_vert,edgesplit}.rs` and the matching mesh ops, including headless `loopcut`/`loopcut_slide` | 3.1 | Subdivide is index-exact. Bevel with 1 segment is exact. Bevel with N segments matches canonically at 1e-4. Loopcut matches the B.7 emulation. |
| 3.5 | Normals, marking, attributes | `crates/frap-op-mesh/src/ops/{flip_normals,normals_make_consistent,mark_*,faces_shade_*,set_sharpness_by_angle,customdata_*,attribute_set,uv_texture_*}.rs`, `crates/frap-op-mesh/src/ops/geometry_*.rs`, `crates/frap-op-mesh/src/ops/transform_edge_{crease,bevelweight}.rs` | 3.1 | The attribute presence and removal rules [ops-mesh §2.5]. |
| 3.6 | Edit-mode transforms | `crates/frap-op-mesh/src/transform/**` | 3.1 | translate, rotate, resize, shrink_fatten, edge_slide and vert_slide. Pivots and orientations (`NORMAL` included). Proportional editing falloffs. |
| 3.7 | UV | `crates/frap-uv/`, `crates/frap-op-uv/` | 3.1 | LSCM unwrap of the seamed cube is exact. Cube, cylinder and sphere projection match Blender's headless output. smart_project matches B.5. Packer invariants (in [0,1], no overlap, area ratio). ABF++ is a stretch goal in M3 and is due in M8. |
| 3.8 | Golden M3 | `tests/golden/cases/edit/**`, `tests/golden/cases/uv/**` | 0.7 | At least 120 cases. |

Max parallelism: 8. 3.1 is on the critical path. The others start against its frozen API and become fully testable once it lands. WPs 3.3 to 3.7 can write bmesh-level unit tests against hand-built BMeshes from day 1.

### M4: Modifier stack (shippable: non-destructive modelling; export with `export_apply`)

| WP | Title | Owns | Needs | Acceptance |
|---|---|---|---|---|
| 4.1 | Evaluation and modifier ops | `crates/frap-eval/`, `crates/frap-op-object/src/ops/modifier_*.rs`, `crates/frap-op-object/src/ops/{shade_auto_smooth,subdivision_set}.rs`, `crates/frap-scene/src/funcs/modifiers_*.rs`, `crates/frap-nodes/src/builtin_groups/smooth_by_angle.rs` | 1.1, 1.2 | Fingerprint invalidation tests. `modifier_apply` with reports. `dimensions`, `evaluated_get` and `to_mesh`. Smooth by Angle matches Blender. |
| 4.2 | Simple modifiers | `crates/frap-modifiers/src/types/{mirror,array,solidify,triangulate,weld,displace,edge_split,smooth,cast,simple_deform,wireframe,screw,weighted_normal,mask,build}.rs` | 4.1 contract | Evaluated-mesh goldens per modifier with defaults and with key settings. |
| 4.3 | Subdivision | `crates/frap-subdiv/`, `crates/frap-modifiers/src/types/subsurf.rs` | 4.1 contract | Canonical match with Blender at levels 1 to 3, creases and UV smooth modes. Both backends behind the trait. |
| 4.4 | Bevel and decimate modifiers | `crates/frap-modifiers/src/types/{bevel,decimate}.rs`, `crates/frap-bmesh/src/ops/decimate_*.rs` | 3.4, 4.1 | Bevel limit methods (ANGLE default, WEIGHT via `bevel_weight_edge`). Decimate COLLAPSE, UNSUBDIV and DISSOLVE within tolerance. |
| 4.5 | Boolean | `crates/frap-boolean/`, `crates/frap-modifiers/src/types/boolean.rs`, `crates/frap-op-mesh/src/ops/intersect*.rs` | 4.1 | MANIFOLD and EXACT match canonically. Attribute transfer (UV, material). FLOAT aliases to EXACT with a WARNING. |
| 4.6 | Integration | `crates/frap-gltf-blender/src/export/apply.rs`, `crates/frap-render-scene/src/evaluated.rs`, `tests/golden/cases/modifiers/**` | 4.1 | `export_apply=True` goldens. Renders show modifiers. |

Max parallelism: 6.

### M5: Full materials and EEVEE-like renderer (shippable: every glTF material extension round-trips; PBR renders with shadows, HDRI and AgX)

| WP | Title | Owns | Needs | Acceptance |
|---|---|---|---|---|
| 5.1 | Lowering: all extensions | `crates/frap-material/src/patterns/{clearcoat,sheen,transmission,volume,ior,specular,iridescence,anisotropy,dispersion,unlit,occlusion,gltf_output_group}.rs`, `crates/frap-gltf-blender/src/export/materials/ext_*.rs`, `.../import/materials_ext_*.rs`, `.../export/images_pack.rs` | 2.3, 2.5 | Blender export goldens per extension. ORM packing rules. `ior` written only with transmission, volume or specular [gltf §4.5]. |
| 5.2 | EEVEE core | `crates/frap-render/src/eevee/{pipeline,lights,shadows,taa,film}.rs`, `crates/frap-render/src/shaders/eevee_*.wgsl` | 2.7 | Light-unit golden against Blender EEVEE on a diffuse plane (±5% luminance). Shadow presence and placement tests. |
| 5.3 | World and IBL | `crates/frap-render/src/eevee/ibl/**`, `assets/brdf_lut.bin`, world lowering in `crates/frap-render-scene/src/world.rs` | 5.2 contract | HDRI renders with SSIM ≥ 0.9 against Blender. |
| 5.4 | Shading features and Workbench completion | `crates/frap-render/src/eevee/features/**`, `crates/frap-render/src/workbench/{matcap,xray,wire,outline,cavity}.rs`, `assets/matcaps/` | 5.2 contract | Per-feature visual goldens (frap-blessed, strict) plus loose Blender comparison. |
| 5.5 | Colour management and outputs | `crates/frap-color/src/{agx,filmic,pbr_neutral,aces,looks}.rs`, `crates/frap-op-render/src/output.rs` | 2.7 | All view transforms and looks within ΔE < 1 against Blender on CPU. Formats: PNG 8/16, JPEG, WEBP, OPEN_EXR, HDR. `color_mode` BW/RGB/RGBA. |
| 5.6 | Render goldens | `tests/golden/cases/render_eevee/**`, `tests/render/**` | 5.2 | Suite green on all CI platforms within tolerance. |

Max parallelism: 6.

### M6: Animation, skinning, morph targets (shippable: keyframe, NLA, rig and morph; export animated GLB; render animations)

| WP | Title | Owns | Needs | Acceptance |
|---|---|---|---|---|
| 6.1 | Animation data and evaluation | `crates/frap-anim/` | 0.3 | F-curve sampling equals Blender's sampled values (±1e-5) for every interpolation, easing, handle type and extrapolation. NLA blending. |
| 6.2 | Keying and frames | `crates/frap-op-anim/src/{funcs,ops}/{keyframe_*,frame_*,nla_*,fcurves_*}.rs`, `crates/frap-op-anim/src/keying.rs`, `crates/frap-eval/src/animate.rs` | 6.1, 1.3 | `CubeAction`/`OBCube`/"Object Transforms" naming. Shape-key slot `KE…` in the object's action. Material action `NT…`. `frame_set` writes originals. |
| 6.3 | Armatures and pose | `crates/frap-scene/src/id/armature*.rs`, `crates/frap-op-anim/src/ops/{armature_*,pose_*}.rs`, `crates/frap-op-object/src/ops/armature_add.rs`, `crates/frap-eval/src/pose.rs`, `crates/frap-modifiers/src/types/armature.rs`, edit-armature parts of `EditState` | 6.1, 4.1 | Edit, pose and rest bones. Zero-length bone removal. Parenting `ARMATURE_NAME`/`ARMATURE_ENVELOPE` (AUTO in M8). LBS deformation equals Blender. |
| 6.4 | Vertex groups and shape keys | `crates/frap-scene/src/id/key.rs`, `crates/frap-op-object/src/ops/{vertex_group_*,shape_key_*}.rs`, `crates/frap-scene/src/funcs/{vertex_group_*,shape_key_*}.rs`, `crates/frap-bmesh/src/shape.rs` | 3.1, 4.1 | `VertexGroup.add` modes. `weight()` error text. `shape_key_add(from_mix)`. Edit-mode basis propagation. |
| 6.5 | glTF animation, skins and morphs | `crates/frap-gltf-blender/src/{export,import}/{animation/**,skins.rs,mesh/morph.rs}` | 6.1–6.4, 2.5 | Goldens for every `export_animation_mode`, sampling vs CUBICSPLINE, STEP, morph weights, skin IBMs and joints, `KHR_animation_pointer` for material, light and camera properties. |
| 6.6 | Animation render | `crates/frap-op-render/src/ops/render_animation.rs` | 6.2, 2.7 | Frame paths, `frame_step`, and is-set overrides. |

Max parallelism: 6.

### M7: Compression, remaining extensions, contact sheet (shippable: complete GLB feature coverage)

| WP | Title | Owns | Needs | Acceptance |
|---|---|---|---|---|
| 7.1 | meshopt and quantization | `crates/frap-codec-mesh/src/meshopt.rs`, `crates/frap-gltf-blender/src/export/compress_meshopt.rs`, `.../quantize.rs` | 2.5 | EXT and KHR variants decode in Blender's importer. gltfpack property mapping. `frap_mesh_quantization`. |
| 7.2 | Draco | `crates/frap-codec-mesh/src/draco*.rs`, optional `crates/frap-draco-sys/` | 2.5 | Spike gate: Blender imports frap's Draco output, and frap decodes Blender's, within the quantization bound. Otherwise the cc-built fallback (no cmake) is enabled. |
| 7.3 | Texture formats | `crates/frap-codec-image/src/{ktx2,basis}.rs`, `crates/frap-gltf-blender/src/export/images_{webp,ktx2}.rs` | 2.4, 2.5 | `EXT_texture_webp` with fallback. `KHR_texture_basisu` import and render, and export when `basisu-encode` is on. `frap_texture_format`. |
| 7.4 | Remaining extensions | `crates/frap-gltf-blender/src/{export,import}/ext_{variants,instancing,node_visibility,diffuse_transmission}.rs`, variant data in `crates/frap-scene/src/id/scene_gltf_variants.rs` | 2.5 | `KHR_materials_variants` with the same data model as Blender's add-on (scene variant list plus per-primitive mapping). `EXT_mesh_gpu_instancing` import (and export behind `frap_*`). `KHR_node_visibility` behind `frap_*`. |
| 7.5 | Contact sheet and agent conveniences | `crates/frap-op-render/src/ops/contact_sheet.rs`, `crates/frap-op-frap/src/ops/{scene_summary,select_by_normal}.rs` | 2.7 | Snapshot of the sheet layout. Tiles equal individual renders. |

Max parallelism: 5.

### M8: Parity hardening and extras (continuous)

| WP | Topic |
|---|---|
| 8.1 | ABF++ and SLIM (`frap-uv`) |
| 8.2 | Bevel multi-segment corner exactness and index order |
| 8.3 | Remaining P1/P2 operators by namespace: one WP per namespace, so they parallelise |
| 8.4 | `ed.undo`/`ed.redo` via the snapshot ring |
| 8.5 | `ARMATURE_AUTO` heat weights |
| 8.6 | Fuzzing, performance budgets (cold start under 30 ms without GPU; 100k-face edit op under 200 ms) |
| 8.7 | `frap-lua` spike against the Session API |

### Critical path and staffing

```
0.3 ─► 0.4 ─► 0.6 ─► 1.3 ─► (M1 exit) ─► 3.1 ─► 3.3/3.4 ─► 4.4 ─► (M4 exit)
 └─► 0.2 (gen) ───────────────────────┘          2.1 ─► 2.5 ─► 5.1 / 6.5 ─► 7.x
                                                  2.7 ─► 5.2 ─► 5.3/5.4
```

- M2 and M3 can overlap: M3 needs only M1 and its own core.
- M5 (renderer and materials) and M6 (animation) can run in parallel once M2 and M4 are done.
- With 8 agents, the steady-state occupancy is about 6 to 8 WPs in flight.

---

## 24. Risks and mitigations

| Risk | Impact | Mitigation in this architecture |
|---|---|---|
| Index-order parity is costly per op | Goldens flaky, agents confused | Pool reproduces mempool. Kernel names mirror Blender's for 1:1 ports. Canonical comparison by default. Exact order only for the order-stable list, extended per op as parity is earned. |
| Young pure-Rust crates (`manifold-rust`, `opensubdiv-rs`, `draco-core`) | Correctness or maintenance | Each sits behind a frap trait with an exact version pin and golden coverage. Fallbacks: own Catmull-Clark, `cc`-built Draco, EXACT via the manifold robust engine. |
| Bevel (about 8k lines in Blender) and UV parametrizer parity | Schedule | Bevel is split across M3 (1 segment) and M8 (corner exactness). UV packer parity is tested by invariants only. Licence decision D1 determines whether porting is allowed. |
| EEVEE look vs Blender | Visual mismatch | Shared `LoweredMaterial`. LUT-baked view transforms (exact). Light units verified by targeted goldens. Loose SSIM against Blender plus strict self-goldens. |
| wgpu major releases every 3–6 months | Churn | wgpu is confined to `frap-render`, upgraded at most twice a year, and `RenderScene` has no wgpu types. |
| Contract changes ripple through parallel work | Merge storms | Contracts freeze per milestone. RFC process. Additive-only changes inside a milestone. `#[non_exhaustive]`. |
| linkme registration dropped by the linker | Missing operators at runtime | `frap-ops-all::link()` references each crate. A startup self-check compares the registry size against `schema/implemented/*.toml` (test plus debug assert). |
| Blender behaviours unverified in research (U1–U10, ops-object §7) | Wrong defaults | Each is assigned to the WP that touches it, and must be closed with a golden before that WP exits. |
| COW snapshot cost on huge meshes | Latency | Only touched datablocks are cloned, and only touched attribute buffers within them. Edit ops copy the BMesh once per command, which is what Blender's edit-mode undo does too. |

---

## 25. Decisions the user must make

| # | Decision | Options | Recommendation (buildability view) | Effect on plan |
|---|---|---|---|---|
| D1 | frap licence | (a) **GPL-3.0-or-later**: may port Blender's C/C++ (polyfill/beautify, bmo_subdivide, bmesh_bevel, decimate, uvedit_parametrizer, the packer, Suzanne's table). (b) **MIT/Apache-2.0**: clean-room reimplementation from behaviour, papers and goldens. | (a) is the fastest and lowest-risk route to parity: WPs 3.3, 3.4, 3.7, 4.4 and 8.1–8.2 become ports instead of research. All the adopted dependencies are GPL-3-compatible (MIT, Apache-2.0, Zlib, BSD, MPL-2.0). `opensubdiv-rs`'s TOST licence must be checked; the own Catmull-Clark fallback covers it. | With (b), add about 30% to the M3, M4 and M8 algorithm WPs, and accept tolerance-based goldens where exact ports would have matched. |
| D2 | `primitive_monkey_add` | Port the vertex table (needs D1=a) or ship a frap-designed monkey under the same op name with a `frap_note` | Follows D1 | WP1.4 |
| D3 | Default strictness for goldens | Canonical with a tolerance vs index-exact | Canonical by default, with an exact list per op | WP 1.8 / 3.8 policy |
| D4 | Initial scene | Factory (A1) vs empty | Factory (A1), with `--empty` | none |
| D5 | `CYCLES` handling | Alias to EEVEE with a warning (A6) vs error | Alias | none |

---

## Appendix A: Example session

```
$ frap <<'EOF2'
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete()
mesh.primitive_cylinder_add vertices=16 radius=0.5 depth=2 location=0,0,1
bpy.context.object.modifiers.new("Bevel", 'BEVEL')
bpy.context.object.modifiers['Bevel'].width = 0.05
bpy.data.materials.new("Steel")
bpy.data.materials['Steel'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1
bpy.context.object.data.materials.append(bpy.data.materials['Steel'])
bpy.ops.object.camera_add(location=(4,-4,3), rotation=(1.1,0,0.785))
bpy.context.scene.camera = bpy.data.objects['Camera']
bpy.ops.export_scene.gltf(filepath='//out/part.glb', export_apply=True)
bpy.ops.frap.render_contact_sheet(filepath='//out/sheet.png')
bpy.ops.wm.save_as_mainfile(filepath='//out/part.frap')
EOF2
{"seq":1,"line":1,"kind":"op","op":"object.select_all","ok":true,"status":"FINISHED","reports":[],"context":{"mode":"OBJECT","active_object":"Cube","selected_objects":["Camera","Cube","Light"]}}
{"seq":2,"line":2,"kind":"op","op":"object.delete","ok":true,"status":"FINISHED","reports":[{"type":"INFO","message":"Deleted 3 object(s)"}],"changes":{"removed":["bpy.data.objects['Camera']","bpy.data.objects['Cube']","bpy.data.objects['Light']"]},"context":{"mode":"OBJECT","active_object":null,"selected_objects":[]}}
{"seq":3,"line":3,"kind":"op","op":"mesh.primitive_cylinder_add","ok":true,"status":"FINISHED","result":{"object":{"$ref":"bpy.data.objects['Cylinder']","type":"MESH"}}, ...}
...
```
