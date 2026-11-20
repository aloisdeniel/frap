# frap architecture (definitive)

Status: **normative**. Version 1.2, 2026-11-16 (v1.1 revised after the adversarial review; v1.2 applies RFC 0001 and freezes the M1 W0 contracts; see the Decision logs at the end). This document is the single source of truth for implementers. Where it disagrees with `docs/design/proposal-*.md` or `docs/research/*.md`, this document wins. Changes go through an RFC in `docs/design/rfc/` (see §21.8).

Target: Blender **5.2 LTS** Python API (verified against local Blender 5.2.4, build `cd5dc428cce1`). Toolchain: Rust 1.97 stable.

Companion documents:
- `docs/OPERATORS.md`: checklist of every operator, RNA struct and node type in scope, with the milestone it lands in.
- `docs/research/*.md`: the evidence base. Cited here as [OBJ], [MESH], [RNA], [GLTF], [RUST].
- `docs/design/proposal-fidelity.md` [FID] and `docs/design/proposal-buildable.md` [BLD]: the two proposals this document merges.

---

## Contents

0. Requirements recap
1. Decision record: which proposal won on each axis, and what was grafted
2. Principles and normative rules
3. Workspace and crate layout
4. Spec pipeline: Blender dumps to generated Rust
5. Core data model
6. Meshes: `Mesh`, `BMesh`, evaluated mesh
7. Evaluation and the modifier stack
8. Context, selection and modes
9. RNA: typed property paths (worked example: `Object.location`)
10. Operator registry (worked example: `mesh.primitive_cube_add`)
11. Front-ends: bpy-line and short form (worked example: one call, two syntaxes)
12. Session and transactions
13. JSON protocol
14. Schema export (`frap --describe`)
15. The `.frap` project format
16. Materials and shader node trees
17. glTF/GLB import and export
18. Animation, armatures, shape keys
19. Renderer
20. Testing
21. Conventions
22. Milestone plan
23. Risks
24. Decisions pending user sign-off
Appendix A. Facts verified for this document
Decision log (review of v1.0)

---

## 0. Requirements recap

These are fixed user decisions. They are not re-opened here.

| Area | Requirement |
|---|---|
| Input | A stdin stream of statements. Two syntaxes: literal bpy lines (`bpy.ops.mesh.primitive_cube_add(size=2, location=(0,0,1))`, `bpy.data.objects['Cube'].location = (1,2,3)`) and a short form (`mesh.primitive_cube_add size=2 location=0,0,1`). No variables and no loops. Both are thin front-ends over **one** typed operator registry and **one** typed RNA property-path model. Lua (mlua) may be added later. |
| Reads | A property path can be read (a bare path line, or `print(...)`). The value comes back as JSON. |
| Output | One JSON response per statement on stdout. On error: a JSON error, state unchanged, the stream continues. (Agents can also use the request-framed `frap serve --jsonl` and `frap mcp` modes over the same `Session`, §13.1.) |
| Users | LLM agents first. `frap --describe` exports a JSON manifest / JSON Schema of all operators and properties. |
| Context | Full Blender context: selection, active object, Object/Edit (and Pose) modes, edit-mode element selection with vert/edge/face select modes, 3D cursor, scene and view layer. |
| Modelling | All `mesh.primitive_*`, transforms, duplicate/join/parent/origin/apply, edit-mode ops on a BMesh-like structure, the non-destructive modifier stack, UV ops, shading and sharpness. |
| glTF | Create and edit all standard GLB features: static core, animation, skinning, morph targets, the KHR/EXT extensions listed in [GLTF §3], Draco and meshopt. Import is glTF/GLB only. Operator and parameter names are Blender's (`import_scene.gltf`, `export_scene.gltf`). |
| Materials | Blender shader node trees, restricted to the node patterns Blender's glTF exporter understands. |
| Native format | `.frap`: a zip container with a diffable `scene.json`, binary buffers and original images, schema-versioned, saved and loaded with `wm.save_as_mainfile` / `wm.open_mainfile`. |
| Rendering | `render.render(write_still=True)` driven by `scene.render.*`. A wgpu EEVEE-like engine (PBR, IBL/HDRI, shadow maps) and a Workbench engine (solid/matcap/wireframe/x-ray). Animation frames. A namespaced multi-view contact sheet. macOS Apple Silicon, Linux with a GPU, Windows. No GPU-less requirement. |
| Testing | Golden tests compare frap with real Blender 5.2 running the same script. They are skipped when Blender is missing. |

---

## 1. Decision record

Two proposals were written. **Fidelity-first** [FID] makes Blender the specification for every observable behaviour, including stale evaluation and element index order. **Buildability-first** [BLD] structures the code so many agents can build it in parallel, with generated metadata and one file per concept. The merged architecture takes this stance:

> **Blender is the specification for everything an agent can observe**: names, defaults, enum items, error classes and messages, selection side effects, element order where it has been proven. **Buildability governs the internal structure**: crate layering, registration, file ownership, contracts. Where reproducing Blender would only reproduce a footgun that no script can rely on (stale reads), frap is *fresher* than Blender, documents the deviation, and normalises the Blender side in golden tests.

| # | Axis | Winner | Why (short) | Grafted from the other |
|---|---|---|---|---|
| 1 | Overall stance | Hybrid (above) | Fidelity of surface is the product; buildability is the only way to ship it with parallel agents. | Fidelity tiers T0–T4 from [FID] become the measurement system for everything. |
| 2 | Crate layout | [BLD] layered crates | Format and algorithm crates (`frap-gltf`, `frap-uv`, `frap-subdiv`, codecs, `frap-geom`, `frap-color`) do not depend on the scene, so each is built and tested by one agent alone. | [FID]'s Blender naming inside crates: BMesh kernel names, `bmesh.ops` slot names, io_scene_gltf2 module names. |
| 3 | Registration | [BLD] `linkme` distributed slices | Adding an operator, RNA function or modifier means adding one file. No central list to fight over. | [FID]'s explicit registry object, kept as the runtime index built from the slices, plus a startup self-check (§10.5). |
| 4 | Blender metadata | Tie: both generate from dumps | Nobody types a property name or default by hand. | [FID]: `spec/` directory, poll matrix A–E, fresh-instance defaults. [BLD]: generated *plain data structs* with RNA bindings for Camera, Light, RenderSettings, modifier settings and similar. |
| 5 | ID storage | [BLD] `IdMap` of `Arc<T>` keyed by `IdKey`, global generation counter, automatic `ChangeLog` | Copy-on-write snapshots, change tracking and cache invalidation all come from one mechanism. | [FID]'s verified `bpy.data` order (case-insensitive, non-natural, insertion-stable) and implicit sharing of heavy arrays. |
| 6 | User counts | [BLD] computed on demand | Cannot drift. Scenes are small. | — |
| 7 | Derived values (`matrix_world`, `dimensions`) after a property write | [BLD] always fresh | Blender's staleness is a footgun; no correct script relies on it. | [FID]'s probes (Appendix A) are kept as documentation. The golden runner calls `view_layer.update()` before derived-value reads on the Blender side (not while time is dirty, §20.3). |
| 8 | Mesh reads in Edit Mode | [BLD] fresh reads, through a **cached read-only view** of the edit BMesh (§6.3), never by writing to `Main` | Same reason as 7; a read must never change the scene, `is_dirty` or the eval fingerprint. | New rule: **writes** to mesh element data while that mesh is in Edit Mode are rejected (`RNA_EDITMODE_WRITE`), because Blender silently discards them. |
| 9 | Depsgraph | [BLD] generation-fingerprint lazy cache | No tag API to forget to call. | [FID]: evaluation before every operator's exec, `frame_set` flushing animated values to originals, deterministic relation order and cycle reporting. Time changes apply animation only at evaluation boundaries, as in Blender (§7.3). |
| 10 | RNA implementation | [BLD] `Rna`/`RnaMut` traits, `rna_struct!` macro, generated plain structs, global hooks | Most properties need zero hand-written code. | [FID]: Blender error wording table, `repr`/`path_from_id` byte-identical rules, socket lookup rules, Python-only member list, 4.x compatibility errors with hints. |
| 11 | Operator implementation | [BLD] `#[operator]` function + generated props struct | One file per operator, compile-time checked. | [FID]: invocation pipeline order (lookup, convert, ensure evaluated, poll, transaction, exec), `OpDef` carrying the dumped poll matrix, fidelity tier per operator. |
| 12 | Front-end IR | [FID] semantic `Command` IR (Invoke/Get/Set/Call/Print) | Both syntaxes visibly lower to the same call; a Lua front-end produces the same commands. | [BLD]: the `Input`/`Value` split (parsers never see the scene), nested-call evaluation, corpus-per-file snapshot tests. |
| 13 | Protocol | Merged | — | `ok` flag, `result`, `changes` from [BLD]; `hints`, `state:"unchanged"`, tagged non-finite floats, always-present `context` from [FID]. Error codes are `&'static str` constants owned by the crate that raises them ([BLD]). |
| 14 | `.frap` | Merged (they nearly agree) | — | [BLD]: `mimetype` entry, `manifest.json`, content-addressed buffers, small arrays inline. [FID]: keys are RNA identifiers, every implemented value is written, orphan purge on save. |
| 15 | glTF | Merged | Own serde model, module-by-module mapping of io_scene_gltf2 5.2.41, `frap_*` namespaced options only where Blender has no prop for the feature (audited, §17.2). | [BLD] codec traits and `frap_compat`; [FID] add-on module mapping table. |
| 16 | Materials | [BLD] one `LoweredMaterial` shared by exporter and renderer | The user restricted materials to glTF-exportable patterns, so the glTF-shaped lowering *is* the shading model. Whatever exports correctly renders correctly. | [FID]'s node list for the renderer's supported subset. |
| 17 | Renderer | [BLD] `frap-render-scene` (no wgpu) + `frap-render` | Render extraction is testable without a GPU; wgpu majors touch one crate. | [FID]: view-transform LUTs dumped from Blender, EEVEE light-unit port, Workbench RANDOM colour hash. |
| 18 | Golden tests | Merged | — | [FID]: tiers T0–T4, Blender emulations for headless-impossible ops, the frap-side snapshot taken **through frap's own RNA** using a property list shared with the Blender dumper. [BLD]: `#@frap` / `#@blender` line prefixes, header directives, checked-in fixtures, live mode. |
| 19 | Milestones | [BLD] ownership discipline and waves | Parallel agents need disjoint files. | **M0 walking skeleton** (2 WPs) proves the contracts end to end before they freeze; then M1 is the vertical slice through everything (parser to GLB to PNG). Golden prerequisites land one wave before the packages whose goldens use them. |
| 20 | Licence | Open (§24) | Recommendation GPL-3.0-or-later. | — |
| 21 | Positional operator arguments | New finding (Appendix A) | Blender 5.2 silently *accepts* `bpy.ops.mesh.primitive_cube_add(2)`. frap rejects non-string positionals with `TypeError` (`OP_ARG_POSITIONAL`): a documented deviation that prevents silent no-ops. | — |
| 22 | Agent transport | New (review) | A raw stream can deadlock an agent (open bracket) and has no request ids. `frap serve --jsonl` frames each request as one complete unit with an `id`; `frap mcp` keeps one `Session` per connection (§13.1). Raw stdin stays for humans and scripts. | — |
| 23 | Data references across layers | New (review) | `IdRef<K: IdKind>` uses zero-sized marker kinds declared in `frap-rna`, so low crates (`frap-mesh`, `frap-nodes`) can point at IDs defined in higher crates (§5.1). | — |

---

## 2. Principles and normative rules

### 2.1 Fidelity principles

1. **Spec data comes from Blender, never from memory.** Names, types, defaults, ranges, enum items, flags, node sockets and fresh-instance values are dumped from `$BLENDER_BIN` (§4) and generated into Rust.
2. **Errors are Blender's errors.** The Python exception class and Blender's message text are reproduced. frap adds structured help (`detail`, `hint`, `did_you_mean`) *beside* the message, never inside it.
3. **Extras are namespaced.** frap-only operators live in the `frap.*` namespace. frap-only properties on Blender operators start with `frap_`. frap-only response data lives in the JSON envelope, outside Blender values.
4. **Deviations are explicit.** Every place frap behaves differently from Blender is listed in `docs/deviations.md` (owned by the integrator) and surfaced by `--describe` (`frap_note`, `headless`).
5. **Implement at Blender's level.** If Blender implements an operation on BMesh (bevel, triangulate, decimate), frap does too; if on the flat `Mesh` (array, mirror, solidify, weld), frap does too. Element order follows from that choice.
6. **Measure fidelity.** Every implemented operator declares a fidelity tier; the golden harness enforces it.

| Tier | Meaning | Comparator |
|---|---|---|
| **T0 surface** | Names, props, types, defaults, enum items, poll result, error class and message match Blender. Required of every registered operator and RNA property. | line outcomes + RNA values |
| **T1 index-exact** | Element counts and **index order**, positions (tol 1e-5), selection, active element match index by index. | `mesh_exact` |
| **T2 canonical** | Order-independent equality: position multiset, rotation-normalised face cycles, per-corner attributes keyed by (face, vertex position). | `mesh_canonical` |
| **T3 invariant** | Only invariants: UVs in [0,1], no overlaps, area ratios, manifoldness, counts within a band. | `invariants` |
| **T4 perceptual** | Image similarity (SSIM, ΔE). | `image_ssim` |

Operators start at T2 or T3 and are promoted to T1 when goldens prove index parity. Primitives, uniform subdivide, region/edge/vert extrude, inset, split, poke, `quads_convert_to_tris` (FIXED), flip_normals, merge and delete target T1 from the start [MESH §5.7].

### 2.2 Buildability rules (reviewers reject PRs that break them)

1. **One concept, one file.** Each operator, RNA function hook, modifier evaluator, glTF extension, bmesh operator, primitive generator, material pattern, comparator and golden case is its own file. Directories of such files are included with `automod::dir!` and register through `linkme` slices. **Nobody edits a `mod` list or a `register_all()`.** `automod::dir!` refuses an empty directory, so a per-concept directory that has no concept yet holds a `_dir.rs` placeholder; files whose name starts with `_` are never concepts (the §10.5 registry test skips them).
2. **Generated code is never hand-edited.** Everything under `*/src/generated/` (not `gen/`: `gen` is a reserved keyword in edition 2024, RFC 0001 F-ED-1) and all of `crates/frap-spec-*/` is written by `cargo xtask codegen`. CI runs `cargo xtask codegen --check`.
3. **Workspace dependencies are pre-declared** in root `[workspace.dependencies]` (§3.5). Members use `foo.workspace = true`. A new external dependency needs the integrator.
4. **Contracts freeze per milestone.** Types and traits marked `// CONTRACT` (§3.4) change only through an RFC merged before dependent work starts. Additive changes (new variants on `#[non_exhaustive]` enums, trait methods with defaults) are allowed.
5. **Stubs first.** Each milestone from M1 on starts with a Wave 0 that makes every new public signature compile (bodies `todo!()` or `Err(NotImplemented)`). Downstream work waits on *signatures*, never on implementations.
6. **Tests are per file.** One golden case per `.py`, one fixture per case, one CLI transcript per `.trycmd`, `insta` snapshots named after the file under test.
7. **stdout belongs to the protocol.** Only `frap-cli` writes to stdout. Logs go to stderr through `tracing`.
8. **No observable `HashMap` order** (§21.4).
9. **FFI budget.** Default features build with `cargo` plus a C/C++ compiler only. No cmake, no libclang, no network in `build.rs`. Allowed `cc`-built C/C++: `meshopt`, `libwebp-sys`. Optional, off by default: `basisu_c_sys` (libclang).
10. **Young crates behind frap traits**, pinned with `=x.y.z`: `manifold-rust` (`BooleanSolver`), `opensubdiv-rs` (`Subdivider`), `draco-core` (`DracoCodec`).
11. **No dev-dependency cycles.** A crate never dev-depends on a crate that depends on it (e.g. `frap-ops-mesh` → `frap-session` → `frap-ops-all` → `frap-ops-mesh` would build two copies of `frap-ops-mesh`, duplicate `linkme` entries and mismatch `OperatorDef` types). `cargo xtask check-deps` (CI) rejects them. Operator behaviour tests live in `frap-golden` cases or in `crates/frap-ops-<x>-tests` (a test-only crate that depends on `frap-session`); inside an ops crate only pure unit tests are allowed.
12. **Portable transcendentals.** Geometry and math crates call `libm` (`libm::sinf`, `cosf`, `powf`, `atan2f`, …), never `f32::sin` and friends, which go to the platform libm and differ in the last bit across OSes (§21.4). Clippy `disallowed-methods` enforces it in `frap-math`, `frap-geom`, `frap-mesh`, `frap-bmesh`, `frap-uv`, `frap-subdiv`, `frap-anim`, `frap-modifiers`.

---

## 3. Workspace and crate layout

### 3.1 Repository tree

```
frap/
├─ Cargo.toml                 # [workspace], [workspace.dependencies], profiles     (integrator)
├─ rust-toolchain.toml        # channel = "1.97"
├─ deny.toml                  # cargo-deny: licences (GPL-3 compatible), advisories, bans
├─ .cargo/config.toml         # `cargo xtask` alias
├─ spec/
│  ├─ current_milestone       # the milestone whose operators get typed props structs (§4.2)
│  ├─ blender-5.2/            # COMMITTED Blender dumps (source of truth, §4)
│  │  ├─ ops/<namespace>.json
│  │  ├─ rna/<Struct>.json
│  │  ├─ nodes/<bl_idname>.json
│  │  ├─ modifiers/<TYPE>.json
│  │  ├─ startup.json         # factory startup scene, exact arrays
│  │  ├─ primitives/<op>.json # reference meshes for every primitive with default and varied props
│  │  └─ messages.json        # captured error / report strings
│  └─ overlay/                # hand-written facts Blender cannot dump
│     ├─ dynamic_enums.toml   # supersets: render.engine, mode_set.mode, modifier_add.type, …
│     ├─ python_only.toml     # Object.children, users_collection, Mesh.from_pydata, keyframe_insert signature, …
│     └─ compat_4x.toml       # removed 4.x members -> AttributeError message + 5.x replacement hint
├─ tools/blender/             # Python run INSIDE Blender
│  ├─ dump_ops.py dump_rna.py dump_nodes.py dump_modifiers.py dump_startup.py dump_primitives.py
│  ├─ bake_view_luts.py make_gltf_fixtures.py
│  ├─ golden_runner.py        # executes a golden case statement by statement
│  ├─ dump_state.py           # writes GoldenState JSON (reads tests/golden/props.toml)
│  └─ emulations.py           # Blender-side equivalents for frap.* and headless-extended ops
├─ xtask/                     # cargo xtask check-deps | budgets | codegen [--check] | golden [bless|live|report] | luts | describe-diff
├─ assets/                    # generated, checked in: luts/*.cube, matcaps/, studio_lights/, brdf_lut.bin, font/
├─ crates/                    # see 3.2 (plus test-only crates frap-ops-<x>-tests, rule 2.2.11)
├─ tests/
│  ├─ golden/{cases,fixtures}/<area>/…   # one .py per case, one .blender.json per case
│  ├─ golden/props.toml       # property list both dumpers snapshot
│  ├─ data/{gltf-samples,gltf-blender}/  # Khronos samples; Blender-exported fixture GLBs (make_gltf_fixtures.py)
│  └─ cli/<milestone>/<name>.trycmd
└─ docs/{ARCHITECTURE.md, OPERATORS.md, deviations.md, agents.md, design/, research/}
```

### 3.2 Crates and responsibilities

Layers: a crate may depend only on crates in **lower** layers. The table itself draws two same-layer edges, which are the only exceptions: `frap-lang` → `frap-rna` (L1) and `frap-ops-all` → every `frap-ops-*` (L10). `cargo xtask check-deps` enforces the table (§20.5).

| Layer | Crate | Responsibility | Key public items |
|---|---|---|---|
| L0 | `frap-math` | Blender math conventions on top of `glam`: `eul_to_mat3` (XYZ = Rz·Ry·Rx), compatible Euler, quaternions w-first, `mat4_to_loc_rot_size`, axis-angle, `BliRng` (drand48-style LCG), shortest-f32 formatting. | `EulerOrder`, `Euler`, `decompose`, `BliRng` |
| L0 | `frap-macros` | Proc-macros, one file each: `rna_struct!`, `#[rna_func]`, `#[rna_hook]` (RNA tier, needed in M1 W1/W2) and `#[operator]`, `#[modifier]`, `#[bmesh_op]` (operator tier). | — |
| L1 | `frap-rna` | Identity (`IdKey`, `IdKind` + one zero-sized marker per ID type, `IdRef<K>`, `IdAny`, `IdCode`, `IdHeader`, `IdType`, `EmbeddedSlot`), metadata (`StructDef`, `PropDef`, `FuncDef`, `EnumItem`), values (`Input`, `Value`), `coerce()`, naming (`unique_name`), `RnaPtr`, `IdNames`, traits (`Rna`, `RnaMut`, `RnaColl`, `StructRef`, `Item`, `Bulk`, guard types), operator spec types (`OperatorSpec`, `OpArgs`, `Prop<T>`), `FrapError`, `PyExc`, `ErrorCode`, `Span`, `Report`, `ReportType`, `serde_buf`, `HOOK_DECLS`, `ERROR_CODES`. | §5.1, §9 |
| L1 | `frap-geom` | `BLI_polyfill_2d` + beautify, Newell normals, corner/face/vertex normals, kd-tree, MikkTSpace wrapper (`bevy_mikktspace`), spatial hash for welding. | |
| L1 | `frap-color` | Colour spaces, sRGB transfer, view transforms (LUT application over a log2 shaper), looks, exposure/gamma, dither. CPU only. | `display_transform()` |
| L1 | `frap-lang` | Hand-written lexer, bpy-line parser, short-form parser, semantic IR (`Stmt`, `Command`, `Expr`). Depends only on `frap-rna` (`Input`, `Span`). | §11 |
| L2 | `frap-spec-rna` | **Generated.** `StructDef` statics, `PropIdx` consts, RNA enums. Data crates depend only on this one. | §4 |
| L2 | `frap-spec-ops-<group>` | **Generated**, one crate per ops group (`object`, `transform`, `mesh`, `uv`, `anim`, `data`, `io`, `render`, `frap`): `OperatorSpec` statics and typed props structs, **only** for operators that OPERATORS.md assigns to a milestone ≤ the current one (`spec/current_milestone`), plus the glTF settings structs in `io`. | §4 |
| L2 | `frap-spec-catalog` | **Generated.** Every other dumped operator as one deflate-compressed JSON blob, decoded on first use into leaked `&'static OperatorSpec` tables in a `OnceLock` (pointer identity holds for the process). Serves `KnownNotImplemented` and `--describe`. | §4 |
| L3 | `frap-anim` | Action / slot / layer / strip / channelbag / F-curve / NLA data; F-curve evaluation (Bézier handle calc, 13 interpolations, easings, extrapolation); NLA blending. Pure, no scene. | `Action`, `FCurve`, `evaluate_fcurve`, `evaluate_anim` |
| L3 | `frap-gltf` | glTF 2.0 serde model (lossless for unknown extensions/extras), GLB container, accessor codec (normalized, sparse, quantized, strides), `BufferBuilder`, validator, one file per typed extension. Blender-agnostic. | `Document`, `Glb`, `Extension` |
| L3 | `frap-imbuf` | Image decode/encode: PNG 8/16, JPEG, WebP (lossless pure Rust; lossy via `webp` feature), HDR, EXR, TGA, BMP; KTX2 read + Basis transcode (features). `ImageCodec` trait. | `DecodedImage`, `EncodeFormat` |
| L3 | `frap-codec-mesh` | `DracoCodec` (draco-core, fallback `-sys`), `MeshoptCodec` (meshopt). | |
| L3 | `frap-uv` | Parametrizer on its own `ParamMesh`: LSCM, ABF++, SLIM (`faer`), smart project, cube/cylinder/sphere projection, island packer, average scale. | |
| L3 | `frap-subdiv` | `Subdivider` trait; own Catmull-Clark with creases and face-varying UVs; optional `opensubdiv-rs` backend; Blender `subdiv_mesh` output ordering pass. | |
| L3 | `frap-boolean` | `BooleanSolver` trait; `manifold-rust` backend (MANIFOLD, EXACT initially); attribute transfer by source face id. | |
| L4 | `frap-mesh` | `Mesh` (struct-of-arrays attributes on 4 domains), primitives (one generator per file), looptris, corner split, shading attributes, `from_pydata`, RNA bindings for `Mesh` and every element struct. | §6.1 |
| L4 | `frap-nodes` | `NodeTree`/`Node`/`Socket`/`Link`, node-type table (from generated defs), dynamic socket availability per node type, built-in node groups ("Smooth by Angle", "glTF Material Output"), RNA bindings. | §16.1 |
| L5 | `frap-bmesh` | `BMesh` with `BLI_mempool`-identical LIFO pools, kernel Euler ops with Blender names, CustomData, select flush/history, walkers, `from_mesh`/`to_mesh`, one file per `bmesh.ops` operator. | §6.2 |
| L6 | `frap-scene` | `Main`, `IdMap<T>`, every ID type except Mesh/NodeTree/Action (incl. `Library` and `PointCloud`), `Scene`/`ViewLayer`/`Base`/`Collection`, `WindowState`, `EditState`, `Context`, `Host` and `Evaluator` traits, factory startup, generated plain structs (RenderSettings, Camera fields, …), RNA bindings and the RNA functions that only touch scene data (`select_set`, `hide_set`, `view_layer.objects.active`, …). | §5, §8 |
| L7 | `frap-modifiers` | `ModifierTypeInfo` registry, one evaluator per file (`types/<type>.rs`). | §7.2 |
| L7 | `frap-material` | `lower(NodeTree) -> LoweredMaterial` (one pattern recogniser per file) and `raise(LoweredMaterial) -> NodeTree`. | §16.2 |
| L8 | `frap-eval` | Depsgraph-lite: evaluated objects, meshes (shape keys + modifiers), world matrices, poses, bounds; animation application at evaluation boundaries (§7.3). Implements `frap_scene::Evaluator`. | §7.3 |
| L8 | `frap-render-scene` | `types` module: `RenderScene`, `Film`, `RenderError`, `RenderRequest`, `GpuOptions`, `trait RenderService` (plain data, no wgpu). `build` module: evaluated scene → `RenderScene` through the `Evaluator` trait (corner-split meshes, `LoweredMaterial`s, images, lights, camera (`BKE_camera_params` port), world, settings). | §19.2 |
| L9 | `frap-op` | Operator framework: `OperatorDef`, `Registry`, invocation pipeline, `OpCtx`, `Services` (render, image cache, path resolver), declarative poll library (`ED_operator_*` names), macro runner, shared helpers (placement, add-primitive-or-merge, multi-object edit loop), extension-point slices (`MODE_HANDLERS`, `TRANSFORM_MODE_HANDLERS`). Re-exports `Report`/`ReportType` from `frap-rna`. | §10 |
| L9 | `frap-render` | wgpu device (lazy), offscreen targets, readback, Workbench and EEVEE-like engines behind `trait Engine`, GPU cache, contact-sheet compositor; implements `frap_render_scene::RenderService`. Only crate that depends on `wgpu`. | §19 |
| L9 | `frap-io-gltf` | Scene ↔ glTF with io_scene_gltf2 5.2.41 semantics; module structure mirrors the add-on (`export/`, `import/`); export extension-point slices (§17.2). | §17 |
| L9 | `frap-project` | `.frap` container: deterministic zip, manifest, content-addressed buffers/images, scene mapping, migrations. | §15 |
| L10 | `frap-ops-object` | `object.*`, `view3d.snap_*` / `view3d.camera_to_view_selected`, `collection.*`, `scene.*`. | |
| L10 | `frap-ops-transform` | `transform.*` for every mode: shared orientation/pivot/proportional core (`core/`), object mode (`object/`), edit mode (`edit/`), pose mode (`pose/`); one file per operator in `ops/` dispatching by mode. | |
| L10 | `frap-ops-mesh` | `mesh.*` (primitives and edit ops), `geometry.*`. | |
| L10 | `frap-ops-uv` | `uv.*`. | |
| L10 | `frap-ops-anim` | `anim.*`, `screen.frame_*`, `armature.*`, `pose.*`, `nla.*`, keyframe RNA functions, keying rules. | |
| L10 | `frap-ops-data` | `material.*`, `image.*`, `world.*`, `bpy.data.*.new/remove/load`, collection/slot/node RNA functions. | |
| L10 | `frap-ops-io` | `import_scene.gltf`, `export_scene.gltf`, `wm.*` file operators. | |
| L10 | `frap-ops-render` | `render.render`, output path rules, `frap.render_contact_sheet`; reaches the GPU only through `OpCtx::services().render()`. | |
| L10 | `frap-ops-frap` | Other `frap.*` operators (`describe`, `select_elements`, `select_by_normal`, `scene_summary`). | |
| L10 | `frap-ops-all` | Aggregator: depends on every `frap-ops-*`; `pub fn link()` references one symbol per crate so the linker keeps every `linkme` entry. | |
| L11 | `frap-session` | `Session`: executor (IR → calls), path resolver, transactions, response builder, JSON value encoding, request framing (`serve`), `Host` and `Services` implementation (owns the lazily created `RenderService` from `frap-render` under feature `gpu`). | §12, §13 |
| L11 | `frap-schema` | `--describe`: manifest, JSON Schema, LLM tool formats. | §14 |
| L12 | `frap-cli` | The `frap` binary: argument parsing, raw stdin loop, `frap serve --jsonl`, `frap mcp` (stdio MCP server), stdout writer, exit codes. | §13.1 |
| L12 | `frap-golden` | `GoldenState` schema, frap-side runner, Blender runner, comparators (one file each), fixtures, bless. Dev only. | §20.3 |

Rules:
- `frap-render` uses only `frap_render_scene::types` and never names a `frap-scene` type (checked by clippy `disallowed-types` in `frap-render`). Render tests use synthetic `RenderScene`s.
- The GPU path is: `frap-ops-render` (L10) → `OpCtx::services().render()` → `&mut dyn RenderService` (trait in `frap-render-scene`, L8) → implemented by `frap-render` (L9) → instance created lazily by `frap-session` (L11). No crate depends on a crate in its own or a higher layer.
- `frap-lang` does not know about operators, RNA or the scene. A future `frap-lua` crate sits beside `frap-lang` and calls the `Session` API (§11.6).
- `frap-scene` does not depend on `frap-eval`; it defines `trait Evaluator` and `trait Host` that higher crates implement (dependency inversion).

### 3.3 Dependency sketch

```
L12  frap-cli (raw, serve, mcp) ─────────────────────── frap-golden
L11  frap-session ── frap-schema
L10  frap-ops-all → frap-ops-{object,transform,mesh,uv,anim,data,io,render,frap}
L9   frap-op    frap-render    frap-io-gltf    frap-project
L8   frap-eval    frap-render-scene (types + build; RenderService trait)
L7   frap-modifiers    frap-material
L6   frap-scene
L5   frap-bmesh
L4   frap-mesh    frap-nodes
L3   frap-anim frap-gltf frap-imbuf frap-codec-mesh frap-uv frap-subdiv frap-boolean
L2   frap-spec-rna    frap-spec-ops-<group>    frap-spec-catalog
L1   frap-rna    frap-geom    frap-color    frap-lang
L0   frap-math   frap-macros
```

### 3.4 Contract surfaces

Contracts marked "M1 W0" are **drafted** in M0 (§22.2), exercised end to end by the walking skeleton and the contract probes, amended by RFC 0001, and only then frozen by the M1 W0 integrator. **Frozen 2026-11-16 (M1 W0, WP 1.0).** Every signature of this table compiles: `crates/frap-ops-all/tests/contracts.rs` pins L0–L10 and `crates/frap-golden/tests/contracts.rs` pins L11–L12 with typed function pointers and bindings, so a signature change breaks those files first. Contract files are created by the W0 integrator and owned afterwards by the WP that implements them; their signatures change only through an RFC (rule 2.2.4).

| Contract | Crate | Section | Frozen at |
|---|---|---|---|
| `IdKey`, `IdKind` + marker kinds, `IdRef<K>`, `IdAny`, `IdCode`, `IdHeader`, `IdType`, `EmbeddedSlot` | frap-rna | §5.1 | M1 W0 |
| `StructDef`, `PropDef`, `FuncDef`, `EnumItem`, `Input`, `Value`, `coerce`, `CoerceCx`, `clamp_report`, `RnaError` | frap-rna | §9 | M1 W0 |
| `Rna`, `RnaMut`, `RnaColl`, `RnaCollMut`, `Field`, `StructRef`, `CollRef`, `Item`, `Bulk`, `RnaMutGuard`, `RnaPtr`, `Seg`, `Root`, `IdNames`, `Resolved` | frap-rna | §9 | M1 W0 |
| `FrapError`, `PyExc`, `ErrorCode`, `Span`, `Report`, `ReportType` | frap-rna | §13.4 | M1 W0 |
| `Prop<T>`, `OpArgs`, `BitSet`, `FromValue`, `OpProps`, `OperatorSpec`, `PollMatrix` | frap-rna / frap-spec-ops-* | §10 | M1 W0 |
| `Stmt`, `Command`, `Expr`, `PathExpr` | frap-lang | §11.2 | M1 W0 |
| `Mesh` (accessor API), `AttributeSet`, `Attribute`, `AttrData`, `Domain` | frap-mesh | §6.1 | M1 W0 |
| `NodeTree`, `Node`, `Socket`, `Link` | frap-nodes | §16.1 | M1 W0 |
| `Main`, `IdMap<T>`, `ChangeLog`, `Context`, `WindowState`, `EditState`, `HostRead`, `Host`, `Snapshot`, `Evaluator`, `RNA_FUNCS`, `RNA_HOOKS`, `RnaHookReg` | frap-scene | §5, §8, §9.5 | M1 W0 |
| `EvaluatedObject` | frap-scene | §7.3 | M1 W0 |
| `OperatorDef`, `Poll`, `PollReq`, `OpCtx` (incl. `split_edit`, `split_eval`, `services`, `snapshot_at`), `Services`, `OpStatus`, `OpError`, `PollFail` | frap-op | §10 | M1 W0 |
| `MODE_HANDLERS` (`ModeHandler`), `TRANSFORM_MODE_HANDLERS` (`TransformModeHandler`) | frap-op | §8, §10.3 | M1 W0 |
| `PRIMITIVE_EXTRACTORS`, `PRIMITIVE_ATTRIBUTE_WRITERS`, `PRIMITIVE_POST_PROCESS`, `MeshExportCx` | frap-io-gltf | §17.2 | M1 W0 |
| `LoweredMaterial`, `MATERIAL_PATTERNS` (`MaterialPattern`, `LowerCx`, `RaiseCx`) | frap-material | §16.2 | M1 W0 (extended M3) |
| `RenderScene`, `RenderService`, `Film`, `RenderRequest`, `GpuOptions`, `Engine`, `Gpu`, `new_service` | frap-render-scene / frap-render | §19 | M1 W0 |
| `Response` JSON shape, `serve` request/response framing | frap-session | §13 | M1 W0 |
| `GoldenState` JSON schema | frap-golden | §20.3 | M1 W0 |
| `BMesh` public API (`types`, `pool`, `kernel`, `query`, `customdata`, `select`, `walkers`, `convert`, `validate`, `ops::<op>::{<Op>In, <Op>Out, <op>}`) | frap-bmesh | §6.2 | M2 W0 |
| `RWorkbenchShading`, `RObject::display_type`, `SheetLayout`/`SheetTile`, `RenderService::composite_sheet` (default body) | frap-render-scene | §19.2, §19.6 | M2 W0 (additive) |
| `DESCRIBE_PROVIDERS` (`DescribeProvider`) | frap-op | §14 | M2 W0 |
| `budget` (`Budget`, `scope`, `check`, `check_now`, `reserve_corners`, `Interrupted`) | frap-math | §12 | M2 W0 |
| `ModifierTypeInfo`, `ModEvalCx`, `MODIFIER_TYPES`, `ModKind`, `ModTypeFlags`, `ModError`, `stack::evaluate`, `registry::find` | frap-modifiers | §7.2 | M4 W0 |
| `Subdivider` (`SubdivTopology`, `SubdivSettings`, `SubdivResult`, `default_subdivider`), `BooleanSolver` (`BoolMesh`, `BoolOptions`, `BoolResult`, `default_solver`) | frap-subdiv / frap-boolean | §3.2, §7.2 | M4 W0 |
| `ParamMesh` (`uv_parametrizer_*`), `PackParams`/`pack_islands`, `project`, `smart` | frap-uv | §3.2 | M3 W0 |
| `MaterialExts` fields, `LoweredMaterial::occlusion_strength` | frap-material | §16.2 | M3 W0 (additive) |
| `MATERIAL_EXT_EXPORTERS`/`MATERIAL_EXT_IMPORTERS` (`material_ext.rs`), `SUPPORTED_EXPORT_PROPS`/`is_supported`, `ExportSettings::{props, evaluated_meshes}`, `Gather::state`, `TextureSlot::{Occlusion, Ext}` | frap-io-gltf | §17.2 | M3 W0 (additive) |
| `Texture` field set (`TextureFields`), `NodesSettings::inputs` (`GnInputs`), generated `id_refs`/`set_id_ref`, `scenes`/`window` functions | frap-scene | §5.6, §7.1 | M3+M4 W0 |

### 3.5 External dependencies (root `[workspace.dependencies]`)

From [RUST §13]; exact patch versions are locked in `Cargo.lock`.

| Crate | Req | Use |
|---|---|---|
| glam | 0.34.3 (`serde`, `approx-05`, `libm`) | f32 math; f64 inside solvers; `libm` routes glam's own transcendentals through libm (rule 2.2.12) |
| wgpu | 30.0.2 (`default-features=false`, `wgsl`, `metal`, `vulkan`, `dx12`, `std`; **not** `static-dxc`, which downloads binaries at build time) | GPU, `frap-render` only. Windows backend order Vulkan, then DX12 with the FXC compiler selected explicitly (§19.2). |
| libm | 0.2 | portable transcendentals (rule 2.2.12) |
| flate2 | 1 (`zlib-rs` backend, shared with `zip`) | `frap-spec-catalog` blob |
| pollster | 1.0.1 | block on wgpu futures |
| image | 0.25.10 (`default-features=false`, png, jpeg, webp, hdr, exr, tga, bmp) | images |
| png 0.18.1, exr 1.74.3 | | 16-bit PNG, multilayer EXR |
| webp | 0.3.1 (feature `webp-lossy`, default on) | lossy WebP |
| ktx2 0.5.0, basisu 0.1.0 (feature `basisu`) | | KTX2/Basis read |
| basisu_c_sys | 0.9.1 (feature `basisu-encode`, off) | Basis encode |
| meshopt | 0.7.0 | meshopt codec, filters |
| draco-core | =2.3.3 (feature `draco`) | Draco (spike, M7) |
| manifold-rust | =0.19.2 | Boolean (spike, M4) |
| opensubdiv-rs | =0.7.0 (feature `subdiv-osd`, on by default since the M4 gate) | Gregory end-cap evaluation of frap's own Catmull-Clark (M34S3-1); full alternative backend `frap_subdiv::osd` |
| bevy_mikktspace | 1.0.0 | tangents |
| spade 2.15.1, earcut 0.4.12 | | CDT for fills; n-gon fallback |
| faer | 0.24.4 (sparse, sparse-linalg) | UV solvers |
| smallvec 1.16.5, rustc-hash 2.1.3, indexmap 2.14.2 (`serde`), smol_str | | containers |
| serde 1.0.229, serde_json 1.0.154 (`preserve_order`, `float_roundtrip`, `raw_value`) | | JSON |
| zip 9.6.0 (`deflate-flate2-zlib-rs`), sha2 0.11.1, bytemuck 1.25.2, half 2.7.1, base64 0.23.1 | | containers, buffers |
| clap 4.6.7, schemars 1.2.3, thiserror 2.0.21, anyhow 1.0.104 (bin only), tracing 0.1.44, tracing-subscriber 0.3.23 | | CLI, schema, errors, logs |
| linkme 0.3.38, automod 1.0.17 | | registration (locked in M1 W0) |
| bitflags 2.13 (`serde`) | | `PropFlags`, `FuncFlags`, `OpOptions`, `BaseFlag`, `ElemFlag` (RFC 0001 F-DEP-1) |
| proc-macro2 1.0.107, quote 1.0.47, syn 2.0.119 (`full`) | | `frap-macros` (M1 W0) |
| toml 1.2.1 | | overlays, `props.toml`, `xtask/budgets.toml` (M1 W0) |
| dev: insta 1.49, approx 0.5.1, proptest 1.12, trycmd 1.2.1, assert_cmd 2.2.2, rstest 0.28, image-compare 0.5, gltf =1.4.1 | | tests |

Not used: `gltf-json` as write model, `nalgebra`, `chumsky`, `csgrs`, `manifold3d`, `draco-rs`, `opensubdiv-petite`, `xatlas-rs*`, `zenwebp` (AGPL) [RUST §13].

### 3.6 Feature flags (on `frap-cli` and `frap-ops-all`)

| Feature | Default | Effect |
|---|---|---|
| `gpu` | on | Builds `frap-render`. Off: `render.render` → `RuntimeError` code `GPU_UNAVAILABLE` ("built without gpu"). Lets parser/modelling CI skip wgpu. |
| `webp-lossy` | on | Lossy WebP encode via libwebp (`cc`). Off: lossless only + WARNING report. |
| `meshopt` | on | meshopt codec. |
| `draco` | on after the M7 spike gate | `draco-core`. `draco-sys` (off): cc-built Google Draco fallback, no cmake. |
| `basisu` | on | KTX2 read + transcode. `basisu-encode` (off): needs libclang. |
| `subdiv-osd` | on (M4 gate, decision log M34S3-1) | frap's own Catmull-Clark evaluates Blender's Gregory end caps (`levels > quality`, `SIMPLE` past level 1) through `opensubdiv-rs` patches. Off: the exact limit there (D-121). |
| `boolean-manifold` | on | `manifold-rust` backend. |

---

## 4. Spec pipeline: Blender dumps to generated Rust

### 4.1 Dumps

All scripts run as `$BLENDER_BIN -b --factory-startup --python tools/blender/<x>.py -- --out spec/blender-5.2/`. The curated namespace list is: `object, mesh, uv, transform, view3d, anim, screen, render, wm, material, image, world, collection, scene, geometry, armature, pose, nla, export_scene, import_scene, ed` (`ed` added in M2 stabilization so that `ed.undo`/`ed.redo` answer `OP_NOT_IMPLEMENTED` with their M8 plan, WP 8.4, instead of `OP_UNKNOWN`; its group is `io`). Every operator in these namespaces is dumped, implemented or not.

| Script | Output | Content |
|---|---|---|
| `dump_ops.py` | `ops/<ns>.json` | `bl_idname`, label, description, `bl_options`; every property (identifier, type, subtype, unit, array shape, default, hard/soft range, enum items, `is_enum_flag`, `is_hidden`, `is_skip_save`, `is_required`, `fixed_type` for macro sub-ops); poll matrix A–E with and without a VIEW_3D override [OBJ §0]. |
| `dump_rna.py` | `rna/<Struct>.json` | `bl_rna` of every struct reachable from `bpy.types` that frap may implement: props, functions (params, returns, keyword-only markers), base. Also **fresh-instance values** read from newly created datablocks, recorded wherever they differ from RNA defaults. |
| `dump_nodes.py` | `nodes/<bl_idname>.json` | Inputs/outputs (`name`, `identifier`, type, default, `enabled` under each mode value, `hide_value`), node properties, default name and width. |
| `dump_modifiers.py` | `modifiers/<TYPE>.json` | Values from `modifiers.new`, default names from `modifier_add` [RNA §5.1]. |
| `dump_startup.py` | `startup.json` | Complete factory state including the default cube's mesh arrays (its vertex order differs from `primitive_cube_add`) and the Layout viewport's `RegionView3D`. |
| `dump_primitives.py` | `primitives/<op>__<variant>.json` | Positions, edges, faces, UVs, attributes for each `mesh.primitive_*` with defaults and several prop variants. Test fixtures. |
| `bake_view_luts.py` | `assets/luts/*.cube` | View transforms and looks baked by rendering an HDR lattice through `image.save_render()` (§19.5). |

Dynamic enums are dumped as Blender reports them in `-b` (often incomplete). The supersets live in `spec/overlay/dynamic_enums.toml` [RNA §0.7].

**Add-on preferences before dumping.** `dump_rna.py` and `dump_ops.py` first set `bpy.context.preferences.addons['io_scene_gltf2'].preferences.KHR_materials_variants_ui = True`. Without it (the `--factory-startup` default, verified) `Scene.gltf2_KHR_materials_variants_variants` and the per-mesh variant properties are not registered, so no RNA path to create a variant would exist. The add-on preferences struct itself is dumped and exposed as `bpy.context.preferences.addons['io_scene_gltf2'].preferences` (§8). Importing a glTF that uses variants turns the preference on, as the add-on does (`blender_gltf.py`, around line 638).

### 4.2 Generated outputs (`cargo xtask codegen`)

| Output | Location | Hand-written counterpart |
|---|---|---|
| `OperatorSpec` statics and one typed props struct per operator assigned to a milestone ≤ `spec/current_milestone` in OPERATORS.md (the integrator bumps the file in each milestone's W0) | `crates/frap-spec-ops-<group>/src/generated/<ns>.rs` | `#[operator]` function in a `frap-ops-*` crate |
| Every other dumped operator (≈ 900 of ≈ 1,100) as a compressed JSON blob, lazily decoded into leaked `&'static` tables | `crates/frap-spec-catalog/src/generated/catalog.json.deflate` | none (`KnownNotImplemented`, `--describe`) |
| `StructDef` statics for every dumped struct (`frap_spec_rna::OBJECT`), one module per struct holding a `const &PropDef` per property (`frap_spec_rna::object::LOCATION`; flat `<STRUCT>_<PROP>` names collide in the full API, RFC 0001 F-GEN-1, and codegen fails on any collision) | `crates/frap-spec-rna/src/generated/<struct>.rs` | `Rna` bindings in the data crates |
| Plain data structs with `Default` = fresh values (read-only props ignored, RFC 0001 F-DEF-7), serde, and generated `Rna` impls: RenderSettings, ImageFormatSettings, ColorManagedViewSettings, ColorManagedDisplaySettings, View3DShading, SceneDisplay, SceneEEVEE, ToolSettings (subset), UnitSettings, View3DCursor, Camera fields, CameraDOFSettings, Light fields per type, every modifier settings struct, Keyframe, NlaStrip fields | `crates/frap-scene/src/generated/plain/*.rs`, `crates/frap-anim/src/generated/*.rs` | none |
| `ModifierType` enum and `ModifierSettings` enum of generated structs | `crates/frap-scene/src/generated/modifier_settings.rs` | evaluators in `frap-modifiers/src/types/*.rs` |
| `NodeTypeDef` per `bl_idname` | `crates/frap-nodes/src/generated/types/*.rs` | `availability/<node>.rs` for nodes whose sockets depend on a mode |
| glTF exporter/importer settings structs (110 / 20 props) | `crates/frap-spec-ops-io/src/generated/{export_scene,import_scene}.rs` | `frap-io-gltf` |

Generation is a committed, hermetic step: no `build.rs`. Dump facts that bpy does not expose come from `spec/overlay/`: dynamic enum supersets (`dynamic_enums.toml`, required for correctness: 22 operator enums are empty in `-b`, RFC 0001 F-DEF-6), Python-only members and keyword-only markers (`python_only.toml`), context-member types (`context_members.toml`, F-DEF-5: `object`, `selected_objects`, … are not RNA props of `Context`; codegen synthesises `READONLY | CONTEXT_MEMBER` PropDefs), and float storage width (`float_storage.toml`, F-DEF-2: default `F32`). A drift test in each `frap-spec-*` crate reloads the JSON and compares it with the compiled tables. `cargo xtask spec-refresh --check` re-dumps from a live Blender and fails on any diff (run on every Blender 5.2.x patch release).

**Build-time budget.** CI records the clean build time of `frap-spec-rna` and of each `frap-spec-ops-<group>` and fails when one exceeds 2× its recorded baseline (`xtask/budgets.toml`). Promoting an operator from the catalog to a typed struct is a one-line change to `spec/current_milestone` plus `cargo xtask codegen`; pointer identity of `&'static OperatorSpec` is stable within a process either way.

**frap-option audit.** A codegen test compares every `frap_*` operator property with the dumped Blender props of that operator: a `frap_*` option whose feature is already covered by a Blender prop (by an allow-listed feature → prop table in `spec/overlay/frap_options.toml`) fails the build. This is how `frap_instancing` was found to duplicate `export_gpu_instances` (§17.2).

### 4.3 Binding checks

Every crate with hand-written `Rna` bindings runs `frap_rna::testing::check_bindings::<T>()` in its tests. It asserts that every bound property exists in the `StructDef` with the same type, array shape and enum items, and that no binding names an unknown property. The set of implemented properties is derived from the bindings themselves, so there is no shared "implemented" list to edit.

`check_bindings` also emits each `=> hook` / `hook_coll` binding into the `HOOK_DECLS` slice. Because hooks are registered in higher crates, the **completeness test** lives in `crates/frap-ops-all/tests/registry.rs` (everything linked): every `HOOK_DECLS` entry has exactly one `RNA_HOOKS` entry and vice versa, every `(srna, func)` in `RNA_FUNCS` is unique, and every operator idname in `OPERATORS` is unique (§10.5).

---

## 5. Core data model

### 5.1 Identity (`frap-rna/src/id.rs`, CONTRACT)

```rust
/// Session-unique, never persisted, never reused (not even after rollback).
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub struct IdKey(NonZeroU64);
impl IdKey { pub fn next() -> IdKey; }                          // process-global counter (F-ID-3)

/// Blender's 2-letter ID codes; also the prefix of action slot identifiers ("OBCube", "KEKey").
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
#[non_exhaustive]
pub enum IdCode { OB, ME, MA, CA, LA, WO, IM, AR, AC, SC, GR, KE, NT, TE, LI, PT }
impl IdCode { pub const fn collection(self) -> &'static str; }  // repr of a dynamic IdAny (F-ID-1)

/// Zero-sized marker per ID type, declared here (L1) so that any crate can name an ID
/// type without depending on the crate that defines its data (frap-mesh points at
/// materials and shape keys, frap-nodes at images, all defined in frap-scene, L6).
pub trait IdKind: Copy + Send + Sync + 'static {
    const CODE: IdCode;
    const COLLECTION: &'static str;     // "objects", "meshes", …  (bpy.data attribute)
    const DEFAULT_NAME: &'static str;   // "Object", "Mesh", "Material", …
}
// frap-rna/src/id_kinds.rs: one uninhabited enum per IdCode, e.g.
pub enum ObjectId {}  pub enum MeshId {}  pub enum MaterialId {}  pub enum KeyId {}  pub enum ImageId {}
pub enum NodeTreeId {}  pub enum LibraryId {}  pub enum PointCloudId {}  /* … one per IdCode */
impl IdKind for MaterialId { const CODE: IdCode = IdCode::MA; const COLLECTION: &'static str = "materials";
                             const DEFAULT_NAME: &'static str = "Material"; }

/// Typed pointer between datablocks. Never a name, never an Arc. Clone/Copy/Eq/Hash/Ord/Debug
/// are implemented by hand (derives would add `K:` bounds).
pub struct IdRef<K: IdKind>(IdKey, PhantomData<fn() -> K>);
impl<K: IdKind> IdRef<K> {                                     // F-ID-2
    pub fn from_key(k: IdKey) -> Self; pub fn key(self) -> IdKey;
    pub fn from_any(a: IdAny) -> Option<Self>; pub fn any(self) -> IdAny;   // checked by code
}
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub struct IdAny { pub code: IdCode, pub key: IdKey }

/// Implemented by the data type, in whatever crate defines it. Exactly one data type per
/// kind (checked by a test in frap-ops-all that walks every IdMap of Main).
pub trait IdType: Rna + Clone + Send + Sync + 'static {
    type Kind: IdKind;                  // impl IdType for Material { type Kind = MaterialId; … }
    fn header(&self) -> &IdHeader;
    fn header_mut(&mut self) -> &mut IdHeader;
    /// Outgoing references (BKE_library_foreach_ID_link): users, orphan purge, save, deps.
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny));
    /// Retarget references (remove(do_unlink=True), user_remap).
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>);
    /// Embedded IDs owned by this ID (Material/World.node_tree, Scene.collection).
    fn embedded(&self) -> &'static [EmbeddedSlot] { &[] }
}

/// Embedded IDs have an IdHeader (is_embedded_data = True) but no IdKey and no IdMap entry.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[non_exhaustive]
pub enum EmbeddedSlot { NodeTree /* Material, World */, MasterCollection /* Scene */ }
impl EmbeddedSlot { pub const fn prop_identifier(self) -> &'static str; }   // "node_tree" | "collection" (F-ID-4)

pub struct IdHeader {
    pub name: SmolStr,                       // ≤255 bytes UTF-8, unique within its IdMap per library
    pub use_fake_user: bool,
    pub library: Option<IdRef<LibraryId>>,   // ID.library; None = local
    pub is_linked_packed: bool,              // ID.is_linked_packed (linked from a packed library)
    pub is_embedded_data: bool,              // ID.is_embedded_data
    pub generation: u64,                     // bumped on every mutable access (§5.3); not `gen` (reserved, F-ED-1)
    pub session_uid: u32,                    // deterministic per session, exposed as RNA session_uid
    pub custom: IndexMap<SmolStr, IdProp>,   // ID properties (obj["foo"]) -> glTF extras
}
pub enum IdProp { Int(i64), Float(f64), Bool(bool), Str(SmolStr), IntArray(Vec<i64>), FloatArray(Vec<f64>),
                  Group(IndexMap<SmolStr, IdProp>), List(Vec<IdProp>) }   // #[non_exhaustive]
```

`IdMap<T>` ties the two together: `impl<T: IdType> IdMap<T> { fn get(&self, r: IdRef<T::Kind>) -> Option<&T>; … }`. `frap-mesh` stores `Vec<Option<IdRef<MaterialId>>>` and never needs `frap-scene`.

**Embedded IDs** (verified: `material.node_tree.is_embedded_data == True`, repr `bpy.data.materials['X'].node_tree`; `Scene.collection` and `World.node_tree` likewise). They are stored inline in their owner, addressed as `Root::Embedded { owner, slot }` (§9.3), and mutated only through the owner's `IdMap::get_mut`, so the **owner's** `generation` bumps and the owner is logged as modified. They can carry `AnimData` (`NTShader Nodetree` action slot, §18) and are valid KHR_animation_pointer targets through that root.

**Linked IDs.** Blender links IDs from libraries even in a factory session: `object.shade_auto_smooth` links the packed "Smooth by Angle" group from `geometry_nodes_essentials.blend` and adds two `bpy.data.libraries` entries (verified; the group's `library` is the `.blend.001` one). frap models a minimal `Library` ID (`LI`: `name`, `filepath`, `parent`, `is_archive`, `packed_file` absent) and linked IDs are read-only for RNA writes (`AttributeError`-style Blender wording captured in `messages.json`). Linked built-in groups are created by frap itself from `frap-nodes/src/builtin_groups/`; nothing is read from a `.blend`.

### 5.2 `Main` (= `bpy.data`) (`frap-scene/src/data.rs`, CONTRACT)

The file is `data.rs`, not `main.rs`: Cargo builds a `src/main.rs` next to `lib.rs` as a binary target (RFC 0001 F-ED-2).

```rust
pub struct Main {
    pub objects: IdMap<Object>,     pub meshes: IdMap<Mesh>,          pub materials: IdMap<Material>,
    pub cameras: IdMap<Camera>,     pub lights: IdMap<Light>,         pub worlds: IdMap<World>,
    pub images: IdMap<Image>,       pub armatures: IdMap<Armature>,   pub actions: IdMap<Action>,
    pub scenes: IdMap<Scene>,       pub collections: IdMap<Collection>, pub shape_keys: IdMap<Key>,
    pub node_groups: IdMap<NodeTree>, pub textures: IdMap<Texture>,   pub libraries: IdMap<Library>,
    pub pointclouds: IdMap<PointCloud>,
    pub filepath: Option<String>,               // plain string: camino is not a workspace dependency
    pub is_dirty: bool,
}
impl Main {                                      // F-MAIN-1: the resolver and hooks reach IDs through IdAny
    pub fn id(&self, a: IdAny) -> Option<&dyn Rna>;
    pub fn id_mut(&mut self, a: IdAny, log: &mut ChangeLog) -> Option<&mut dyn RnaMut>;  // via get_mut
    pub fn header(&self, a: IdAny) -> Option<&IdHeader>;
    pub fn collection(&self, attr: &str) -> Option<(Vec<IdAny>, &'static StructDef)>;   // bpy.data.<attr>
}
impl IdNames for Main { … }                      // §9.3

pub struct IdMap<T: IdType> {
    items: IndexMap<IdKey, Arc<T>>,          // iteration order = Blender ListBase order (5.4)
    by_name: BTreeMap<(SmolStr, Option<IdKey> /*library*/), IdKey>,
}
impl<T: IdType> IdMap<T> {
    pub fn get(&self, r: IdRef<T::Kind>) -> Option<&T>;
    pub fn get_arc(&self, r: IdRef<T::Kind>) -> Option<Arc<T>>;     // F-MAIN-2 (mesh_view in object mode)
    /// Arc::make_mut (clones only if a snapshot shares it) + header.generation = GEN.fetch_add(1) + log.modified.
    pub fn get_mut(&mut self, r: IdRef<T::Kind>, log: &mut ChangeLog) -> Option<&mut T>;
    /// coll['Name'], coll.get(): the local ID first, then linked IDs in library order.
    pub fn find(&self, name: &str) -> Option<IdRef<T::Kind>>;
    /// coll['Name', <lib>] tuple key (pinned on 5.2.4 by WP 1.10, `Main::find_key_lib`): `None` finds the
    /// local ID only; a string selects the FIRST library whose `filepath` equals it (Blender's
    /// `BLI_findstring` over `bpy.data.libraries`), else `KeyError: "bpy_prop_collection[id, lib]: lib
    /// filepath '<p>' does not reference a valid library"`; a missing name is `KeyError:
    /// 'bpy_prop_collection[key, lib]: not found'`. So an ID of a second library with the same filepath
    /// (the packed `….blend.001` archive `shade_auto_smooth` creates) cannot be addressed by tuple key,
    /// exactly as in Blender (W2-3, D-15).
    pub fn find_in(&self, name: &str, library: Option<IdRef<LibraryId>>) -> Option<IdRef<T::Kind>>;
    pub fn index_of(&self, r: IdRef<T::Kind>) -> Option<usize>;     // coll.find()
    pub fn at(&self, i: isize) -> Option<IdRef<T::Kind>>;           // coll[i], negative allowed
    pub fn insert(&mut self, requested: &str, value: T, log: &mut ChangeLog) -> IdRef<T::Kind>;  // unique_name among local IDs
    pub fn rename(&mut self, r: IdRef<T::Kind>, requested: &str, mode: RenameMode, log: &mut ChangeLog) -> RenameResult;
    pub fn remove(&mut self, r: IdRef<T::Kind>, log: &mut ChangeLog) -> Option<Arc<T>>;
    pub fn iter(&self) -> impl Iterator<Item = (IdRef<T::Kind>, &T)>;
}

pub struct ChangeLog {                       // per statement; feeds Response.changes
    pub added: Vec<IdAny>, pub removed: Vec<(IdAny, SmolStr /*last name*/)>, pub modified: IndexSet<IdAny>,
    pub renamed: Vec<(IdAny, SmolStr /*old*/)>, pub context_changed: bool,
}
pub enum RenameMode { Never, Always, SameRoot }   // ID.rename(mode=…)
pub enum RenameResult { Unchanged, UnchangedCollision, RenamedNoCollision, RenamedCollisionAdjusted, RenamedCollisionForced }
```

### 5.3 Copy-on-write, generations, change tracking

- Every datablock is an `Arc<T>`. Heavy arrays inside datablocks are `Arc<[T]>`/`Arc<Vec<T>>` (Blender's implicit sharing). Changing `obj.location` copies a ~1 KB `Object`, never a mesh.
- `GEN: AtomicU64` is process-global and lives **outside** any snapshot, so rollback never reuses a generation. Evaluation caches key on generations (§7.3).
- `IdMap::get_mut` is the **only** mutable access path. It bumps `generation` and logs the ID as modified. Change sets in responses are therefore exact and free. Embedded IDs are reached through their owner's `get_mut` (§5.1).
- Heavy per-ID runtime caches (mesh normals, looptris, decoded image pixels) are **not** cloned with the ID: their `Clone` returns an empty cache (§6.1), so a COW copy can never carry caches computed for the pre-write data.
- `State::clone()` (§12) clones the `IdMap` tables in O(#IDs); data is shared until written.

### 5.4 Naming and ordering (verified)

- `frap_rna::naming::unique_name(taken: &dyn Fn(&str) -> bool, requested: &str, max_bytes: usize, empty_default: &str) -> SmolStr` implements [RNA §0.1–0.2]: truncate on a UTF-8 boundary; split at the last `.NNN`; take the **lowest free** suffix ≥ 1, zero-padded to 3 digits (`Foo.001` requested while taken → `Foo.003` if `.001`/`.002` are taken; a duplicate of `Bar.5` → `Bar.001`); empty → type default. ID names allow 255 bytes; sub-data names (bones, vertex groups, shape keys, modifiers, nodes, UV layers, view layers, `parent_bone`) allow 63.
- Assigning `.name` to a taken name renames the **assigned** ID to `.001`; the other keeps its name. `ID.rename(name, mode='NEVER'|'ALWAYS'|'SAME_ROOT')` returns Blender's status enum.
- `bpy.data.<coll>` order is Blender's sorted ListBase: **case-insensitive, non-natural comparison, equal keys in insertion order** (verified [FID App. A Q3]: `['a10','a9','b','B','Cube','Cube.002','Cube.010','Cube.9']`). Renames re-sort. Two other orders exist and are kept separate: view-layer **base order** (selection order, §8) and the glTF exporter's **natural, case-insensitive** root order (§17). Linked IDs sort after local ones (WP 1.10). The tuple-key rule is pinned in §5.2 (`find_in`); a linked ID reprs as `bpy.data.<coll>['Name']` unless a local ID of the same name shadows it, and only then as `['Name', '<library filepath>']` (D-15; a shadowed ID of a second library with the same filepath has no repr that resolves back, as in Blender). T0 goldens for filepath keys and local/linked ordering arrive with `object.shade_auto_smooth` (WP 1.21/1.26); until then unit tests cover them.
- **Object and data names are uniquified independently**, each in its own `IdMap`. Verified: delete the factory Cube, then `primitive_cube_add()` gives object `'Cube'` and mesh `'Cube.001'`, because the orphan mesh `'Cube'` still exists. The same holds for cameras, lights, armatures and every `*_add` operator (golden `tests/golden/cases/primitives/cube_add_after_delete.py`, T0).
- A local ID may share its name with a linked ID (verified: a local `node_groups.new("Smooth by Angle", …)` coexists with the linked one; `bpy.data.node_groups['Smooth by Angle']` returns the local one). Uniqueness is per `(name, library)`.

### 5.5 Users and orphans

User counts are computed on demand by scanning `for_each_ref` over all IDs (plus fake users). `ID.users` is an RNA hook that calls it. `object.delete` and `object.join` leave 0-user datablocks, as Blender does [OBJ §1.11]; `bpy.data.<coll>.remove(id, do_unlink=True)` deletes immediately. Save drops orphans (§15).

### 5.6 Datablock types

One file each under `crates/frap-scene/src/id/` (except Mesh in `frap-mesh`, NodeTree in `frap-nodes`, Action in `frap-anim`). **Struct fields are named after the RNA identifier whenever a 1:1 property exists** (`location`, `rotation_euler`, `matrix_parent_inverse`; `r#type` for `type`), so serde keys in `.frap` equal RNA identifiers. Enum properties are stored as their RNA identifier (`SmolStr`), validated by `coerce` on write. **The field sets of the M1 types (Object, Scene, Collection, Material, Camera, Light, World, Image, Library) are frozen at M1 W0** with Blender-verified constructors (`Object::new`, `Material::new`, `Camera::new`, `Light::new(name, type)`, `World::new`, `Image::new(name, w, h)`, `Collection::new`/`master`, `Scene::new`), so W2 packages compile against them before WP 1.10 lands; their generated plain sub-structs (`CameraFields`, `LightFields`, `RenderSettings`, …) are W0 stand-ins with the same names and field types until WP 1.1 generates them (decision log W0-21). Every animatable ID also has `animation_data: Option<AnimData>`.

| Type | Key content (Blender names) |
|---|---|
| `Object` | `data: Option<IdAny>` (fixed at creation, determines read-only `type`), `location`, `rotation_mode`, `rotation_euler`/`_quaternion`/`_axis_angle` (all kept), `scale`, `delta_*`, `parent`, `parent_type`, `parent_bone`, `parent_vertices`, `matrix_parent_inverse`, `modifiers: Vec<Modifier>`, `material_slot_overrides: Vec<SlotOverride{link, material}>` (see below), `active_material_index`, `vertex_groups_active_index`, `pose: Option<Pose>`, `mode: ObjectMode`, `hide_viewport`, `hide_render`, `hide_select`, `display_type`, `color`, `empty_display_type`, `empty_display_size`, `active_shape_key_index`, `show_*`, `animation_data: Option<AnimData>` |
| `Mesh` (frap-mesh) | §6.1. Vertex-group **names live on the Mesh** (`vertex_group_names`, DNA-level; RNA exposes them through `Object.vertex_groups`, verified: linked duplicates share them) |
| `Material` | `node_tree: NodeTree` (embedded, always present, name "Shader Nodetree"), `surface_render_method` (`DITHERED`/`BLENDED`), `blend_method` (deprecated mirror, fresh `HASHED`), `use_backface_culling`, `diffuse_color`, `metallic`, `roughness` (these three are the **viewport display** values, not the Principled inputs), `alpha_threshold`, `pass_index`, `use_nodes` (deprecated, always True) |
| `Library` | `name`, `filepath`, `parent`, `is_archive`; only the libraries frap's built-in linked groups need (§5.1) |
| `PointCloud` | `position` and `radius` POINT attributes plus generic attributes, `materials`; type, storage and RNA exist from M1 (so `Main` does not change shape later); created by `import_scene.gltf(import_point_as_pointcloud=True)` and exported as POINTS from M3 |
| `Camera`, `Light`, `World` | generated plain fields + hooks: `Camera { fields: CameraFields /* type, lens, sensor_*, shift_*, clip_*, ortho_scale, dof, … */ }` (`angle`↔`lens` hook); `Light { type, fields: LightFields }`, where `LightFields` is the union of every light type's fields as in Blender's DNA, so assigning `light.type` re-types the RNA struct (`PointLight`/`SunLight`/`SpotLight`/`AreaLight`) and keeps the values; `World { node_tree (embedded), color, use_nodes }` |
| `Image` | `source`, `filepath`, `packed_file: Option<Arc<EncodedImage{bytes, mime}>>`, `file_format`, `generated_*`, `colorspace_settings.name`, `alpha_mode`, `size`; decoded pixels cached outside the snapshot (`Services::images`) |
| `Armature` | `bones` (rest), `collections`, `pose_position`, `display_type`; edit bones live in `EditState` |
| `Action` (frap-anim) | slots, layers, strips, channelbags, F-curves (§18) |
| `Key` | `key_blocks: Vec<ShapeKey{name, value, slider_min, slider_max, relative_key, vertex_group, mute, interpolation, data: Arc<Vec<[f32;3]>>}>`, `use_relative`, `reference_key` |
| `Scene` | `collection` (master, embedded "Scene Collection"), `view_layers`, `camera`, `world`, `frame_start/end/current/step`, `render: RenderSettings`, `view_settings`, `display_settings`, `display: SceneDisplay` (Workbench, incl. `shading`), `eevee`, `cursor`, `tool_settings`, `unit_settings`, glTF variant list (M7) |
| `Collection` | `objects: Vec<IdRef<ObjectId>>`, `children: Vec<IdRef<CollectionId>>`, `hide_*`, `color_tag` |
| `NodeTree` (group, frap-nodes) | built-in groups "Smooth by Angle" and "glTF Material Output" |
| `Texture` | only what DISPLACE needs (`IMAGE`, `NONE`) |

**Material slots are derived, not stored.** Blender keeps `len(ob.material_slots) == len(ob.data.materials)` for every object sharing the data (`BKE_object_materials_test`). frap stores only the per-object overrides (`link`, `material`) and pads them lazily when read: the slot count is always `len(data.materials)`, `MaterialSlot` is a proxy struct (§9.4) built from the object and its data, and writing `slot.link = 'OBJECT'` or `slot.material` grows `material_slot_overrides` to the slot index. `mesh.materials.append` therefore touches only the mesh, and glTF import or `.frap` load cannot leave objects out of sync. Overrides beyond the data's slot count are kept (as Blender keeps `ob->mat` entries) but are not visible.

Fresh defaults come from instance dumps, not RNA defaults, because they differ [RNA header]: e.g. `object.light_add` gives 10 W (factory Light is 1000 W), `Light.shadow_soft_size` 0.0 on `lights.new`, `Material.blend_method == 'HASHED'`.

### 5.7 Factory startup

`frap_scene::startup::factory() -> (Main, WindowState)` builds `bpy.data` and the window state (the session wraps them into its `State`, §12) from `spec/blender-5.2/startup.json` (compiled in with `include_str!`): Camera, Cube (factory vertex order), Light (POINT 1000 W), `Material`, `World`, `Collection`, `ViewLayer`, render settings (EEVEE, 1920×1080, 100%, `/tmp/`, PNG RGBA 8), AgX, cursor at origin, active and selected = Cube. `frap --empty` and `wm.read_homefile(use_empty=True)` give `startup::empty() -> (Main, WindowState)`. `wm.read_homefile()` and `wm.read_factory_settings()` reset to the factory state. Only `Dots Stroke` (grease pencil material) is omitted, documented in `docs/deviations.md`.

---

## 6. Meshes: `Mesh`, `BMesh`, evaluated mesh

Three representations, as in Blender:

| Representation | Type | Used when | Crate |
|---|---|---|---|
| Original mesh | `Mesh` (SoA attributes) | object mode, file I/O, RNA element access | frap-mesh |
| Edit mesh | `Arc<BMesh>` in `EditState` | Edit Mode ops | frap-bmesh, frap-scene |
| Evaluated mesh | `Arc<Mesh>` in the eval cache | modifiers applied: render, `export_apply`, `evaluated_get`, `to_mesh`, `dimensions` | frap-eval |

### 6.1 `Mesh` (`frap-mesh/src/mesh.rs`, CONTRACT)

```rust
pub struct Mesh {
    pub header: IdHeader,
    face_offsets: Arc<Vec<i32>>,                     // len = faces + 1 (polygons[i].loop_start / loop_total)
    attributes: AttributeSet,                        // incl. "position", ".edge_verts", ".corner_vert", ".corner_edge"
    deform_verts: Option<Arc<Vec<DeformVert>>>,      // MDeformVert: SmallVec<[(u32 group, f32 weight); 4]>
    pub materials: Vec<Option<IdRef<MaterialId>>>,   // marker kind from frap-rna (§5.1): no dependency on frap-scene
    pub vertex_group_names: Vec<SmolStr>,
    pub shape_keys: Option<IdRef<KeyId>>,
    pub uv_active: Option<SmolStr>, pub uv_active_render: Option<SmolStr>,
    pub color_active: Option<SmolStr>, pub color_default: Option<SmolStr>,
    pub texspace: TexSpace,
    #[serde(skip)] runtime: MeshRuntime,             // OnceLock caches; never persisted, compared or cloned
}
impl Mesh {
    pub fn attributes(&self) -> &AttributeSet;
    pub fn face_offsets(&self) -> &[i32];
    pub fn deform_verts(&self) -> Option<&[DeformVert]>;
    /// Every geometry write goes through a `&mut self` method that first clears `runtime`.
    pub fn attributes_mut(&mut self) -> &mut AttributeSet;          // clears runtime, then hands out the set
    pub fn set_topology(&mut self, face_offsets: Vec<i32>, corner_verts: Vec<i32>, edges: Vec<[i32; 2]>);
    pub fn deform_verts_mut(&mut self) -> &mut Vec<DeformVert>;
    pub fn corner_normals(&self) -> Arc<Vec<[f32; 3]>>;            // cached; likewise:
    pub fn face_normals(&self) -> Arc<Vec<[f32; 3]>>;
    pub fn vert_normals(&self) -> Arc<Vec<[f32; 3]>>;
    pub fn looptris(&self) -> Arc<Vec<[u32; 3]>>;
    pub fn bounds(&self) -> Option<[[f32; 3]; 2]>;                 // [min, max]; None without vertices
}
impl Clone for MeshRuntime { fn clone(&self) -> Self { MeshRuntime::default() } }   // COW copies start cold
pub struct AttributeSet { attrs: Vec<Attribute> }    // Blender order: by domain, then creation
pub struct Attribute { pub name: SmolStr, pub domain: Domain, pub data: AttrData }
pub enum Domain { Point, Edge, Face, Corner }
pub enum AttrData {                                  // each Arc'd for COW
    Float(Arc<Vec<f32>>), Int(Arc<Vec<i32>>), Int8(Arc<Vec<i8>>), Bool(Arc<Vec<bool>>),
    Float2(Arc<Vec<[f32; 2]>>), Float3(Arc<Vec<[f32; 3]>>), Float4(Arc<Vec<[f32; 4]>>),
    FloatColor(Arc<Vec<[f32; 4]>>), ByteColor(Arc<Vec<[u8; 4]>>),
    Int16x2(Arc<Vec<[i16; 2]>>), Int32x2(Arc<Vec<[i32; 2]>>),
    Quaternion(Arc<Vec<[f32; 4]>>), Float4x4(Arc<Vec<[[f32; 4]; 4]>>), String(Arc<Vec<SmolStr>>),
}
#[derive(Default)]
pub struct MeshRuntime {                             // private to frap-mesh
    corner_normals: OnceLock<Arc<Vec<[f32; 3]>>>, face_normals: OnceLock<Arc<Vec<[f32; 3]>>>,
    vert_normals: OnceLock<Arc<Vec<[f32; 3]>>>, looptris: OnceLock<Arc<Vec<[u32; 3]>>>,
    bounds: OnceLock<Option<[[f32; 3]; 2]>>,
}
```

Cache invariant: no code outside `frap-mesh` can reach a geometry column mutably without going through a method that resets `runtime`, and `Arc::make_mut` on a `Mesh` produces a copy with empty caches. A proptest writes random attributes and checks that cached normals and looptris always equal a fresh computation.

Rules copied from Blender [MESH §1.2–1.3, §2.5]:
- Attribute names: `position`, `sharp_face`, `sharp_edge`, `uv_seam`, `crease_edge`, `crease_vert`, `bevel_weight_edge`, `bevel_weight_vert`, `material_index`, `custom_normal` (INT16_2D, CORNER), UV maps as FLOAT2 CORNER, internal `.`-prefixed (`.select_*`, `.hide_*`, `.uv_select_*`, `.pn.<uv>`).
- Attributes are kept in **creation order** (not grouped by domain), as Blender 5.2.4 does: replacing an attribute keeps its position, removing and re-adding appends. `Mesh::new` creates no attributes (like `bpy.data.meshes.new`); accessors return empty slices.
- `sharp_face`, `sharp_edge` and `uv_seam` are **removed when every value is false only by `bm_to_me`** (`Mesh::remove_all_false_flags`, used by the BMesh conversion). `MeshPolygon.use_smooth`, `MeshEdge.use_seam`, `MeshEdge.use_edge_sharp` and attribute writes are RNA proxies that create the attribute on first write and keep it even when all values become false (`Mesh::set_bool_attribute`, verified on 5.2.4; decision log W1-9).
- `normals_domain` (port of `Mesh::normals_domain`, checked in this order): POINT for a mesh without faces; the domain of `custom_normal` when it is stored on points, faces or corners; FACE if every face is sharp or every edge is sharp; POINT if nothing is sharp; CORNER otherwise. Flags are read with Blender's `lookup_or_default<bool>` semantics (any stored type/domain is converted: `Mesh::lookup_bool`, `sharp_face_flags`, `sharp_edge_flags`). Corner normals are angle-weighted fans split at sharp edges, sharp-face boundaries and non-manifold edges (port of `mesh_normals.cc` into `frap-geom`).
- **Looptris** (glTF export, renderer): quads → `(0,1,2),(0,2,3)`, or `(0,1,3),(1,2,3)` when `is_quad_flip` (verified on non-planar quads [FID App. A P7/P8]); n-gons → `BLI_polyfill_2d` **without** beautify. BMesh triangulate ops use polyfill **with** beautify.
- `frap-mesh/src/primitives/<name>.rs`: one generator per primitive returning `Mesh` in Blender's exact vertex, edge, face and UV order, with `scale`/`size`/`radius` baked into positions, `sharp_face` all true, a `UVMap` layer when `calc_uvs`. Verified against `spec/blender-5.2/primitives/*.json` at T1.
- `frap-mesh/src/split.rs::corner_split()` splits vertices by (vertex, corner normal rounded to 4 dp, every UV, every corner colour, tangent) for the glTF exporter and the renderer [GLTF §7.3]; the default cube gives 24 vertices.
- RNA bindings for `vertices`, `edges`, `polygons`, `loops`, `loop_triangles`, `attributes`, `uv_layers`, `color_attributes`, `vertex_colors` (legacy, creates `Col` BYTE_COLOR CORNER), `vertex_normals`, `polygon_normals`, `corner_normals` live in `frap-mesh/src/rna/<struct>.rs`. Element structs (`MeshVertex`, `MeshEdge`, `MeshPolygon`, `MeshLoop`, `MeshUVLoop`, attribute `data[i]`) are **proxies** over the struct-of-arrays columns (§9.4), and the collections support `foreach_get`/`foreach_set` (§13.3).

### 6.2 `BMesh` (`frap-bmesh`, CONTRACT from M2)

```rust
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)] pub struct VertId(u32);
// EdgeId, LoopId, FaceId likewise. Ids are pool slots; iteration order == slot order.

/// BLI_mempool emulation: alloc pops the most recently freed slot (LIFO), else appends.
pub struct Pool<T> { slots: Vec<Option<T>>, free: Vec<u32>, live: u32 }
impl<T> Pool<T> {
    pub fn alloc(&mut self, v: T) -> u32;
    pub fn free(&mut self, i: u32);
    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)>;   // ascending slots, skipping free
}

bitflags! { pub struct ElemFlag: u16 { const SELECT=1; const HIDDEN=2; const SMOOTH=4; const SEAM=8;
                                       const TAG=16; const INTERNAL_TAG=32; } }
pub struct BMVert { pub co: [f32; 3], pub no: [f32; 3], pub e: Option<EdgeId>, pub flag: ElemFlag }
pub struct BMEdge { pub v: [VertId; 2], pub l: Option<LoopId>, pub disk: [DiskLink; 2], pub flag: ElemFlag }
pub struct DiskLink { pub next: EdgeId, pub prev: EdgeId }
pub struct BMLoop { pub v: VertId, pub e: EdgeId, pub f: FaceId,
                    pub next: LoopId, pub prev: LoopId, pub radial_next: LoopId, pub radial_prev: LoopId }
pub struct BMFace { pub l_first: LoopId, pub len: u32, pub no: [f32; 3], pub mat_nr: i16, pub flag: ElemFlag }

pub struct BMesh {
    pub verts: Pool<BMVert>, pub edges: Pool<BMEdge>, pub loops: Pool<BMLoop>, pub faces: Pool<BMFace>,
    pub vdata: CustomData, pub edata: CustomData, pub ldata: CustomData, pub pdata: CustomData, // columns by slot
    pub select_mode: SelectMode,             // VERT|EDGE|FACE bits
    pub select_history: Vec<BMElem>,         // last = active element
    pub act_face: Option<FaceId>,
    pub shape_layers: Vec<ShapeLayer>,       // one per key block while editing
}
```

| Module | Content |
|---|---|
| `kernel.rs` | Blender names so ports are 1:1 and creation order matches: `vert_create`, `edge_create`, `face_create`, `face_create_verts`, `vert_kill`, `edge_kill`, `face_kill`, `split_edge_make_vert` (SEMV), `join_edge_kill_vert` (JEKV), `split_face_make_edge` (SFME), `join_face_kill_edge` (JFKE), `vert_splice`, `edge_splice`, `face_normal_flip`, `vert_separate`, `elem_index_ensure` |
| `query.rs` | iterators (`vert_edges`, `edge_loops`, `face_loops`, …), `edge_exists`, `face_exists`, manifold tests |
| `select.rs` | `select_flush_mode`, `select_set`, history remap, mode conversion table [MESH §2.2, §3.3] |
| `customdata.rs` | typed columns, interpolation (`loop_interp_from_face`, `data_interp_from_verts`) |
| `walkers.rs` | edge loop, edge ring, island, shell walkers (`BMW_*`) |
| `convert.rs` | `BMesh::from_mesh(&Mesh, &FromMeshParams)` / `to_mesh(&self, template: &Mesh) -> Mesh` mirroring `BM_mesh_bm_from_me`/`bm_to_me`, including all-false attribute removal (`Mesh::remove_all_false_flags`, only here) and shape-key basis propagation. Check attribute order after an edit-mode round trip: Blender 5.2.4 reorders them (`my_flag`, `UVMap`, `position` after exit) |
| `validate.rs` | disk/radial cycle consistency; used by proptest after random op sequences |
| `ops/<name>.rs` | **one file per `bmesh.ops` operator**, named and slotted as in Blender [MESH App. C]: `pub fn extrude_face_region(bm: &mut BMesh, i: &ExtrudeFaceRegionIn) -> Result<ExtrudeFaceRegionOut, BmError>`. Each file starts with a *create/kill order note* derived from Blender's source and [MESH App. B]. |

Edit-mode `mesh.*` operators are thin adapters: gather the selection, call the bmesh op, update selection and history, report counts.

### 6.3 Edit-mode semantics

- `object.mode_set(mode='EDIT')` enters multi-object edit: every selected object of the active object's type, deduplicated by data. Each mesh gets `BMesh::from_mesh` into `EditState.meshes`. Leaving writes back with `to_mesh` and drops the BMesh.
- `EditState.meshes: BTreeMap<IdKey, EditMesh>` with `EditMesh { bm: Arc<BMesh>, bm_gen: u64, view: ViewCache }`. `OpCtx::bmesh_mut` bumps `bm_gen` and resets `view`.
- **Reads** of `Mesh` element data while it is in Edit Mode go through `Host::edit_mesh_view(mesh) -> Arc<Mesh>`: a read-only `to_mesh` conversion cached in `EditMesh.view` and keyed by `bm_gen`. `Main` is never touched, no `gen` is bumped, `is_dirty` and the eval fingerprint do not change, and reads need no transaction. Repeated reads cost one conversion per edit. Reads are therefore always fresh (deviation from Blender, §1 #8); the golden runner calls `obj.update_from_editmode()` on the Blender side before reads.
- **Writes** to mesh element data or attributes (`vertices`, `edges`, `polygons`, `loops`, `attributes`, `uv_layers[...].data`, `from_pydata`) while that mesh is in Edit Mode raise `RuntimeError` code `RNA_EDITMODE_WRITE` with a hint naming `bpy.ops.object.mode_set(mode='OBJECT')`. Non-geometry writes (`mesh.name`, `mesh.materials.append`) are allowed.
- `tool_settings.mesh_select_mode` is the scene default, applied on entering edit mode and by `mesh.select_mode`.

---

## 7. Evaluation and the modifier stack

### 7.1 Modifier data

```rust
pub struct Modifier {                            // frap-scene/src/id/modifier.rs
    pub name: SmolStr,                           // ≤63 bytes, unique per object
    pub show_viewport: bool, pub show_render: bool, pub show_in_editmode: bool, pub show_on_cage: bool,
    pub show_expanded: bool, pub is_active: bool, pub use_pin_to_last: bool, pub persistent_uid: u32,
    pub settings: ModifierSettings,              // GENERATED: Subsurf(SubsurfSettings) | Bevel(..) | Nodes(..) | …
}
```

`Modifier::srna()` refines to `SubsurfModifier`, `BevelModifier`, … so generated property tables apply. Every Blender modifier type exists as data (settings round-trip in `.frap`); only types with an evaluator change geometry. NODES settings hold `node_group` and `properties.inputs: IndexMap<SmolStr /*identifier*/, GnInput{name, value, type, attribute_name}>`, giving the 5.2 path `mod.properties.inputs.Input_1.value`; `mod["Input_1"]` raises Blender's `TypeError: this type doesn't support IDProperties` [RNA §0.6].

### 7.2 Modifier plugins (`frap-modifiers`, CONTRACT from M4)

```rust
pub enum ModKind { OnlyDeform, Constructive, Nonconstructive }
pub struct ModifierTypeInfo {
    pub ty: ModifierType,                                  // generated enum
    pub kind: ModKind,
    pub flags: ModTypeFlags,                               // SUPPORTS_EDITMODE | ENABLE_IN_EDITMODE | …
    pub depends_on: fn(&ModifierSettings) -> SmallVec<[IdAny; 2]>,
    pub is_disabled: fn(&ModifierSettings) -> Option<&'static str>,  // "Boolean: no operand"
    pub eval: fn(&Modifier, Mesh, &mut dyn ModEvalCx) -> Result<Mesh, ModError>,
}
#[linkme::distributed_slice] pub static MODIFIER_TYPES: [ModifierTypeInfo];

pub trait ModEvalCx {                                      // implemented by frap-eval (geometry.rs)
    fn main(&self) -> &Main;                               // M4 W0: textures, collections, node groups
    fn object(&self) -> &Object;
    fn object_ref(&self) -> IdRef<ObjectId>;               // M4 W0
    fn frame(&self) -> f32;                                // M4 W0: DEG_get_ctime (BUILD)
    fn evaluated_mesh(&mut self, ob: IdRef<ObjectId>) -> Result<Arc<Mesh>, ModError>;
    fn object_matrix_world(&mut self, ob: IdRef<ObjectId>) -> Mat4;
    fn pose(&mut self, arm: IdRef<ObjectId>) -> Result<Arc<EvaluatedPose>, ModError>;
    fn image(&mut self, im: IdRef<ImageId>) -> Option<Arc<DecodedImage>>;
    fn mode(&self) -> EvalMode;                            // Viewport | Render
    fn report(&mut self, r: Report);
    fn subdivider(&self) -> &dyn Subdivider;
    fn boolean_solver(&self) -> &dyn BooleanSolver;
}

// crates/frap-modifiers/src/types/mirror.rs — the whole unit of work
#[modifier(MIRROR, kind = Constructive, flags = SUPPORTS_EDITMODE | ENABLE_IN_EDITMODE)]
fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> { /* … */ }
```

A type without an evaluator passes the mesh through and adds a WARNING report: `Modifier 'Remesh' (REMESH) is not evaluated by frap; passed through`. `--describe` lists evaluator support per type. `#[modifier]` also takes `depends_on = <fn>` and `is_disabled = <fn>` (defaults `no_deps`, `never_disabled`); `ModKind` has Blender's five `ModifierTypeType`s and `ModTypeFlags` the `eModifierTypeFlag_*` bits. Modifier ID pointers are user-counting exactly when they do not point at an object (`IDWALK_CB_USER` vs `IDWALK_CB_NOP` in every `MOD_*.cc`), are remapped on removal, and round-trip in `.frap` (generated `id_refs`/`set_id_ref`, M4 W0).

Implementation level (principle 5): ARRAY, MIRROR (bisect via BMesh), SOLIDIFY, WELD, EDGE_SPLIT, DISPLACE, WEIGHTED_NORMAL, ARMATURE, SMOOTH, CAST, SIMPLE_DEFORM, SCREW, WIREFRAME on `Mesh`; BEVEL, TRIANGULATE, DECIMATE through the shared `frap-bmesh` ops; SUBSURF through `frap-subdiv`; BOOLEAN through `frap-boolean`; NODES dispatches on the node group's **(library basename, name)**, e.g. `("geometry_nodes_essentials.blend", "Smooth by Angle")`, never on the name alone, so a user's local group with the same name is not mistaken for the built-in one. Only "Smooth by Angle" is supported (other groups: modifier error `Unsupported node group "X" (frap supports: Smooth by Angle)`).

### 7.3 Depsgraph-lite (`frap-eval`)

```rust
pub struct EvalCache { objects: FxHashMap<(IdKey, EvalMode), CacheEntry> }  // never iterated for output
struct CacheEntry { fingerprint: u64, result: Arc<EvaluatedObject> }
pub struct EvaluatedObject {                                                // frap-scene, CONTRACT
    pub matrix_world: Mat4, pub mesh: Option<Arc<Mesh>>, pub bounds: Option<[[f32; 3]; 2]>,
    pub pose: Option<Arc<EvaluatedPose>>, pub modifier_status: Vec<(SmolStr, ModifierStatus)>,
    pub warnings: Vec<Report>,
}
impl frap_scene::Evaluator for EvalCache {
    fn object(&mut self, host: &dyn HostRead, ob: IdRef<ObjectId>, mode: EvalMode) -> Result<Arc<EvaluatedObject>, EvalError>;
    fn depsgraph_update(&mut self, host: &dyn HostRead);     // view_layer.update(), evaluated_depsgraph_get()
}
```

- **Fingerprint** = hash of the object's `gen`, its data's `gen`, its shape-key `gen`, the edit BMesh generation (if any), the scene frame, and recursively the fingerprints of its parent and every `depends_on` target. Every write bumps a `gen`, so invalidation is automatic. Cycles are detected (Blender's `Dependency cycle detected` WARNING) and evaluated in `bpy.data` order.
- **Mesh pipeline per object**: base mesh (edit BMesh converted, or original) → shape-key mix (relative) → modifiers in stack order with `use_pin_to_last` last, skipping `!show_viewport` (`!show_render` in Render mode) and, in Edit Mode, `!show_in_editmode` → normals lazily.
- **Timing**: operators evaluate before poll/exec (verified: `parent_set` sees the fresh parent matrix [FID Q1]). RNA reads of derived values (`matrix_world`, `dimensions`, `bound_box`, evaluated meshes) always evaluate first: **always fresh** with respect to the current originals. Such reads never apply animation and never write originals.
- **Animation is applied at evaluation boundaries, as in Blender.** Verified: after `scene.frame_current = 6` an animated `location` still reads the old value; it changes only at the next evaluation (`view_layer.update()`, the next operator). So the `frame_current` setter only stores the frame and sets `State.time_dirty`. Pending animation is written into the **original** data through RNA paths (`frap-eval/src/animate.rs`) at exactly these boundaries: operator pipeline step 4 (§10.2), `view_layer.update()`, `scene.frame_set()`, `context.evaluated_depsgraph_get()` / `evaluated_get()`. The resulting `gen` bumps invalidate the cache. Consequence (matches Blender): `frame_current = 10; obj.location.x = 1; obj.keyframe_insert("location")` keys the pre-boundary y and z values.
- **Render and export never write originals.** Blender's render depsgraph and the exporter's `frame_set(0)` … restore re-apply animation, which overwrites manual edits of animated channels in the originals. frap instead clones `State` (COW, §5.3), applies animation at the requested frame on the clone, renders or exports from the clone and drops it. Output pixels and GLB values match Blender; the originals keep manual edits (deviation, listed in `docs/deviations.md`).

---

## 8. Context, selection and modes

```rust
// frap-scene/src/context.rs (CONTRACT)
pub struct WindowState {                      // part of the snapshot; RNA: bpy.context.window (Window stub)
    pub scene: IdRef<SceneId>,                // window.scene = … switches the context scene (verified in -b)
    pub view_layer: SmolStr,                  // window.view_layer
    pub view3d: VirtualView3D,                // factory Layout viewport matrix + space_data.shading
}
pub struct ViewLayer {                        // inside Scene
    pub name: SmolStr,
    pub bases: Vec<Base>,                     // base order = layer-collection sync order
    pub active: Option<IdRef<ObjectId>>,        // may be unselected (verified)
    pub layer_collection: LayerCollection,    // exclude / hide_viewport / holdout tree
    pub active_layer_collection: LayerCollectionPath,
}
pub struct Base { pub object: IdRef<ObjectId>, pub flag: BaseFlag /* SELECTED | HIDDEN */ }
pub struct EditState {                        // part of the snapshot
    pub meshes: BTreeMap<IdKey /*Mesh*/, EditMesh>,          // §6.3
    pub armatures: BTreeMap<IdKey /*Armature*/, Arc<EditArmature>>,
}

/// No EditState borrow: the mode is on Object.mode, so an operator can hold a Context and
/// a `&mut EditState` at the same time (Host::split_edit / OpCtx::split_edit, §9.5, §10.1).
pub struct Context<'a> { pub main: &'a Main, pub win: &'a WindowState, pub prefs: &'a Preferences }
impl<'a> Context<'a> {
    pub fn scene(&self) -> &'a Scene;                     pub fn view_layer(&self) -> &'a ViewLayer;
    pub fn active_object(&self) -> Option<IdRef<ObjectId>>; pub fn object(&self) -> Option<IdRef<ObjectId>>;
    pub fn selected_objects(&self) -> Vec<IdRef<ObjectId>>; // base order, not click order (verified)
    pub fn selected_editable_objects(&self) -> Vec<IdRef<ObjectId>>;
    pub fn visible_objects(&self) -> Vec<IdRef<ObjectId>>;
    pub fn mode(&self) -> ContextMode;                    // OBJECT | EDIT_MESH | EDIT_ARMATURE | POSE
    pub fn edit_object(&self) -> Option<IdRef<ObjectId>>;
    pub fn objects_in_mode(&self) -> Vec<IdRef<ObjectId>>;
    pub fn objects_in_mode_unique_data(&self) -> Vec<IdRef<ObjectId>>;
    pub fn collection(&self) -> Option<IdRef<CollectionId>>; // where *_add links new objects; None = the
                                                          // scene's master collection (embedded, no IdRef)
    pub fn tool_settings(&self) -> &'a ToolSettings;      pub fn cursor(&self) -> &'a View3DCursor;
    pub fn active_bone(&self) -> Option<BoneRef>;         pub fn active_pose_bone(&self) -> Option<BoneRef>;
    pub fn selected_pose_bones(&self) -> Vec<BoneRef>;    pub fn pose_object(&self) -> Option<IdRef<ObjectId>>;
}
```

- Selection and hiding are **per view layer** (`Base.flag`). `Object.select_set/select_get/hide_set/hide_get/visible_get` are RNA functions on the base of the current view layer. There is no `Object.select` property.
- `bpy.context` is read-only. Writes go through `view_layer.objects.active = …`, `select_set`, `hide_set`. `bpy.context.object = x` gives `AttributeError: bpy_struct: Context property "object" is read-only` (verified).
- `Object.mode` is stored per object (`OBJECT`, `EDIT`, `POSE`); `context.mode` is derived (`OBJECT`, `EDIT_MESH`, `EDIT_ARMATURE`, `POSE`).
- **Virtual 3D View.** frap behaves as if one VIEW_3D area exists with the factory Layout viewport matrix (from `startup.json`), so `align='VIEW'` matches Blender (`(1.1088, 0, 1.1549)`), `camera_add` sets `scene.camera` when it is None, and `V`-poll operators pass [OBJ §1.7]. `bpy.context.space_data` resolves to a stub exposing `shading` and `region_3d.view_matrix`. `bpy.context.window` resolves to a `Window` stub exposing writable `scene` and `view_layer` (Blender has a window in `-b --factory-startup`; `bpy.context.window.scene = bpy.data.scenes['S2']` switches the context scene, verified). Other UI members are `None`.
- **Scenes.** `Main.scenes` may hold several scenes from M3 (`bpy.data.scenes.new`, `scene.new`, glTF `scenes[]` import, `export_scene.gltf(use_active_scene=…)`). `context.scene` is always `window.scene`.
- **Preferences.** `bpy.context.preferences` is read-only except `addons['io_scene_gltf2'].preferences` (dumped add-on preferences, §4.1), which is writable and saved in `.frap`.
- The 3D cursor (`scene.cursor`) feeds unset primitive `location`, `origin_set(type='ORIGIN_CURSOR')`, pivot `CURSOR`, `merge(type='CURSOR')`.
- **Mode switching is an extension point.** `object.mode_set` (`frap-ops-object/src/ops/mode_set.rs`, owned once) dispatches to `ModeHandler { mode, object_type, enter, exit }` entries in the `MODE_HANDLERS` slice: `modes/object.rs` (M1), `modes/edit_mesh.rs` (M2), `modes/edit_armature.rs` and `modes/pose.rs` (M6). A missing handler gives `OP_NOT_IMPLEMENTED` naming the mode. Later milestones add files, never edit the dispatcher.
- **Poll is declarative.** Poll functions come from a shared library in `frap-op/src/poll.rs` with Blender's `ED_operator_*` names (`objectmode`, `editmesh`, `active_object`, `active_editable_mesh`, `scene_editable`, `posemode`, `editarmature`). Each is a `Poll { requires: &'static [PollReq], check }` where `PollReq` is data (`Mode(EDIT_MESH)`, `ActiveType(MESH)`, `ActiveEditable`, `HasSelectedObjects`, `SceneHasCamera`, …). Failure text is Blender's: `Operator bpy.ops.mesh.bevel.poll() failed, context is incorrect` (verified), or the custom message (`Object has no vertex groups`). frap adds `error.detail.requires` (all requirements), `error.detail.unmet` (the failing ones) and a `hint` naming the exact fix, e.g. `bpy.ops.object.mode_set(mode='EDIT')`. `--describe` publishes `requires` per operator.
- Agents can ask before calling: `bpy.ops.mesh.bevel.poll()` returns `False` (verified) as a `get`-kind response with `detail.unmet`; `bpy.ops.mesh.bevel.get_rna_type()` returns the operator's describe fragment; `bpy.ops.mesh.bevel.idname()` returns `"MESH_OT_bevel"` (§11.2).

---

## 9. RNA: typed property paths

### 9.1 Metadata (`frap-rna/src/def.rs`, CONTRACT)

```rust
pub struct StructDef {
    pub identifier: &'static str,              // "Object", "BevelModifier", "ShaderNodeBsdfPrincipled"
    pub base: Option<&'static StructDef>,      // "ID", "Modifier", "Node"
    pub name: &'static str, pub description: &'static str,
    pub props: &'static [PropDef],             // own props only; lookup walks the base chain
    pub funcs: &'static [FuncDef],
    pub name_property: Option<&'static str>,   // used for coll['Name'] lookup and listing
    pub is_id: bool,
}
pub struct PropDef {
    pub identifier: &'static str, pub name: &'static str, pub description: &'static str,
    pub kind: PropKind, pub subtype: Subtype, pub unit: Unit,
    pub array: ArrayShape,                     // Scalar | Fixed(n) | Matrix(r, c) | Dynamic
    pub flags: PropFlags,                      // bitflags: READONLY HIDDEN SKIP_SAVE ENUM_FLAG REQUIRED OUTPUT ANIMATABLE
                                               // PYTHON_ONLY DYNAMIC_ENUM NEVER_NULL + (F-DEF-1) ARGUMENT_OPTIONAL
                                               // DEPRECATED PATH_SUPPORTS_BLEND_RELATIVE CONTEXT_MEMBER SKIP_PRESET
    pub deprecated: Option<&'static Deprecation>,   // F-DEF-3 (Material.use_nodes: DeprecationWarning report)
}
pub struct Deprecation { pub note: &'static str, pub version: [u16; 3], pub removal_version: [u16; 3] }
pub struct EnumItem { pub identifier: &'static str, pub name: &'static str, pub description: &'static str, pub value: i32 }
pub enum PropKind {
    Bool  { default: Defaults<bool> },
    Int   { default: Defaults<i64>, hard: (i64, i64), soft: (i64, i64) },
    Float { default: Defaults<f64>, hard: (f64, f64), soft: (f64, f64), precision: u8,
            storage: FloatStorage /*F32|F64: not dumpable, from spec/overlay/float_storage.toml (F-DEF-2)*/ },
    String{ default: &'static str, max_bytes: u32 /* 0 = unlimited */ },
    Enum  { items: &'static [EnumItem], default: EnumDefault, dynamic: Option<&'static str /*overlay key*/> },
    Pointer    { fixed_type: &'static str, is_id: bool },
    Collection { item_type: &'static str, api: Option<&'static str> /* "ObjectModifiers": new/remove */ },
}
pub struct FuncDef {
    pub identifier: &'static str, pub description: &'static str,
    pub params: &'static [PropDef],            // positional order
    pub returns: &'static [PropDef],
    pub keyword_only_from: Option<u8>,         // keyframe_insert(data_path, *, …); overlay-only (not dumpable, F-DEF-4)
    pub flags: FuncFlags,                      // NO_SELF | USE_SELF_TYPE | PYTHON_ONLY (what bpy exposes, F-DEF-4)
}
```

### 9.2 Values (`frap-rna/src/value.rs`, CONTRACT)

Front-ends produce untyped `Input`. `coerce()` turns it into typed `Value` against a `PropDef`. Getters return `Value`. Parsers therefore never need the scene.

```rust
pub enum Input {
    None, Bool(bool), Int(i64), Float(f64), Str(String),
    Bare(SmolStr),                             // short form unquoted word: valid only for String/Enum/EnumSet targets
    Tuple(Vec<Input>), List(Vec<Input>), Set(Vec<Input>), Dict(IndexMap<String, Input>),
    Math(MathValue),                           // Vector, Euler(order), Quaternion, Matrix, Color
    Ptr(RnaPtr),                               // evaluated path expression (an ID or struct)
}
pub enum MathValue { Vector(Vec<f64>), Euler([f64; 3], EulerOrder), Quaternion([f64; 4]), Matrix(Vec<Vec<f64>>), Color(Vec<f64>) }
pub enum Value {
    None, Bool(bool), Int(i64), Float(f32), Double(f64), Str(SmolStr),
    Enum(SmolStr), EnumSet(BTreeSet<SmolStr>),
    BoolArray(SmallVec<[bool; 4]>), IntArray(SmallVec<[i64; 4]>),
    FloatArray { data: SmallVec<[f32; 16]>, shape: ArrayShape, tag: MathTag },  // Vector|Euler(EulerOrder)|Quaternion|AxisAngle|Matrix|Color|None
    Id(Option<IdAny>),                         // ID pointer, JSON {"$ref"} (F-VAL-4); From<IdRef<K>> for Value
    Ptr(Box<RnaPtr>), Coll(Box<RnaPtr>),       // boxed: Value would be 200 B otherwise (F-VAL-1)
    Tuple(Vec<Value>),
    Structs(Vec<IndexMap<SmolStr, Value>>),    // operator collection props: files=[{"name": "a.glb"}] (F-VAL-3)
}
/// Blender's message prefix depends on the assignment site (F-VAL-2): coerce returns the message
/// without prefix; callers convert with RnaError::into_frap(cx.prefix(), …).
pub enum CoerceCx { RnaSet { owner: &'static str /*struct*/ }, OpArgs { owner: &'static str /*bl_idname*/ } }
pub fn coerce(def: &PropDef, input: Input, cx: &CoerceCx) -> Result<Coerced, RnaError>;
pub struct Coerced { pub value: Value, pub clamped: bool }
```

`coerce` is the **only** place where Blender's assignment rules live [RNA §1.2]: ints accepted for floats; any sequence of the right length for arrays (`ValueError: … sequences of dimension 0 should contain 3 items, not 2`); scalar to array → `TypeError`; enum checked (`enum "FOO" not found in ('A', 'B')`, item list included); enum flags accept set/list/tuple; numbers clamped to the hard range (silently in Blender; frap adds an INFO report built by `frap_rna::clamp_report`, the same for assignments and operator arguments: `frap: size=-5 clamped to hard min 0.0`, `frap: size=10000000000000.0 clamped to hard max 999999995904.0`, and for arrays `frap: location=(…) clamped to hard range [lo, hi]`; values and bounds in Python `repr`, deviation D-03); strings truncated to `max_bytes` on a UTF-8 boundary; `Math` accepted where the shape fits; pointers checked against `fixed_type`; `Bare` accepted for String/Enum/EnumSet only, otherwise `TypeError` with hint `quote it or pass a number`.

### 9.3 Paths (`frap-rna/src/ptr.rs`, CONTRACT)

```rust
pub struct RnaPtr { pub root: Root, pub path: SmallVec<[Seg; 6]>, pub srna: &'static StructDef }
pub enum Root {
    Data /*bpy.data*/, Context /*bpy.context*/, Id(IdAny),
    Embedded { owner: IdAny, slot: EmbeddedSlot },   // material.node_tree, world.node_tree, scene.collection
}
pub enum Seg { Prop(&'static str), Index(i64), Key(SmolStr) }
impl RnaPtr {
    pub fn repr(&self, names: &dyn IdNames) -> String;   // bpy.data.objects['Cube'].modifiers["Bevel"]
    pub fn path_from_id(&self) -> Option<String>;        // modifiers["Bevel"].width  (F-curve data_path form)
}
/// Implemented by Main (F-PTR-1). Linked IDs repr as bpy.data.node_groups['Name', '<lib path>'].
pub trait IdNames { fn id_name(&self, id: IdAny) -> Option<(SmolStr /*name*/, Option<SmolStr> /*library path*/)>; }
pub enum Resolved {
    Struct(RnaPtr),
    Prop { ptr: RnaPtr, prop: &'static PropDef, index: Option<i64>, component: Option<char> /* .x .y .z .w .r .g .b */ },
    Coll { ptr: RnaPtr, prop: &'static PropDef },
    Func { ptr: RnaPtr, func: &'static FuncDef },
}
```

Resolution (`frap-session/src/resolve.rs`):
1. Walk segments read-only. On reaching an ID pointer, **re-root** at `Root::Id`; on reaching an embedded ID, re-root at `Root::Embedded { owner, slot }`. A path never borrows across datablocks. `repr` of an embedded root is the owner's repr plus the slot (`bpy.data.materials['X'].node_tree`), and `path_from_id` on anything inside it is relative to the embedded ID, as in Blender. Test (verified string): the Principled Base Color socket has repr `bpy.data.materials['X'].node_tree.nodes["Principled BSDF"].inputs[0]`.
2. `coll['Name']` uses `name_property`; missing → `KeyError: 'bpy_prop_collection[key]: key "Nope" not found'`. `coll.get(name, default)` returns the default; `coll.find(name)` returns −1. `coll[i]` uses Blender order; out of range → `IndexError: bpy_prop_collection[index]: index 9 out of range, size 3`.
3. Node sockets: `inputs['Base Color']` returns the **first enabled** socket with that name; identifiers are not keys; integer indices work [RNA §0.10].
4. `repr` uses single quotes at the root and double quotes below; `path_from_id` emits socket **indices** (`nodes["Principled BSDF"].inputs[0].default_value`, verified) and is byte-identical to Blender because the glTF exporter pattern-matches on F-curve paths.
5. Math component accessors (`location.x`, `rotation_quaternion.w`, `color.r`) and array indices (`location[1]`, slices `location[0:2]`) resolve to `Resolved::Prop` with `component`/`index`.
6. Unknown attribute → `AttributeError: 'Object' object has no attribute 'foo'` + `did_you_mean`; a 4.x member listed in `spec/overlay/compat_4x.toml` → `AttributeError` with the 5.x replacement as `hint` (e.g. `action.fcurves` → `action.layers[0].strips[0].channelbag(slot).fcurves`).

### 9.4 Traits implemented by data structs (`frap-rna/src/traits.rs`, CONTRACT)

Many RNA structs do not exist in memory: `MeshVertex`, `MeshEdge`, `MeshPolygon`, `MeshLoop`, `MeshUVLoop` and attribute `data[i]` are views into struct-of-arrays columns; `MaterialSlot` is derived from the object and its data; `VertexGroup` lives on the Object but its name on the Mesh; `Context` and `Window` are computed. The traits therefore return **owned proxies** wherever a borrow is impossible:

```rust
#[non_exhaustive]
pub enum Field<'a> {
    Value(Value), Struct(StructRef<'a>), Id(Option<IdAny>), Coll(CollRef<'a>),
    Embedded(EmbeddedSlot, StructRef<'a>),   // re-root at Root::Embedded { owner: <current ID>, slot } (F-TR-1)
    Ptr(RnaPtr),                             // re-root at a canonical pointer: context.tool_settings is
                                             // bpy.data.scenes['Scene'].tool_settings (F-TR-2)
    NeedsHook,                               // `=> hook` / `hook_coll` getter: call RNA_HOOKS (F-TR-3)
}
pub enum StructRef<'a> { Borrowed(&'a dyn Rna), Owned(Box<dyn Rna + 'a>) }          // e.g. MeshVertexRef { mesh: &Mesh, i }
pub enum CollRef<'a>   { Borrowed(&'a dyn RnaColl), Owned(Box<dyn RnaColl + 'a>) }
pub enum Item<'a> { Struct(StructRef<'a>), Id(Option<IdAny>) }   // ID items and None items (F-TR-4)
/// Mutable access is always a guard: either a reborrow of real memory (Box::new(&mut node), through
/// the blanket `impl Rna/RnaMut for &mut T`, F-TR-6) or a proxy such as
/// MeshVertexMut { mesh: &'a mut Mesh, i: usize }, which writes through Mesh's &mut methods (§6.1).
pub type RnaMutGuard<'a> = Box<dyn RnaMut + 'a>;
pub type RnaCollMutGuard<'a> = Box<dyn RnaCollMut + 'a>;
/// Dense bulk buffer (F-TR-7): 4 bytes per scalar; flat row-major, item length = prop.array_len().
pub enum Bulk { F32(Vec<f32>), I32(Vec<i32>), Bool(Vec<bool>) }

pub trait Rna: Send + Sync {
    fn srna(&self) -> &'static StructDef;                       // refined (e.g. BevelModifier)
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError>;
}
pub trait RnaMut: Rna {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError>;
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError>;
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError>;
}
pub trait RnaColl {
    fn item_type(&self) -> &'static StructDef;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool { self.len() == 0 }              // clippy len_without_is_empty (F-TR-8)
    fn key_of(&self, i: usize) -> Option<SmolStr>;             // None for nameless items (vertices, loops)
    fn find(&self, key: &str) -> Option<usize>;                 // node sockets: first enabled match; ID-item
                                                                // collections: None, the resolver uses IdNames
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError>;     // F-TR-4
    /// Canonical repr/path segment of item i (F-TR-5): Key(name) when named, else Index(i); sockets and
    /// material slots override it to Index(i) even when looked up by name (verified).
    fn item_seg(&self, i: usize) -> Seg { … }
    /// Bulk access (foreach_get / slices, §13.3). Default: loop over item(); SoA collections override.
    fn foreach_get(&self, prop: &'static PropDef, range: Range<usize>) -> Result<Bulk, RnaError> { … }
}
pub trait RnaCollMut: RnaColl {
    fn item_mut(&mut self, i: usize) -> Result<RnaMutGuard<'_>, RnaError>;
    /// Wrong length: RuntimeError "internal error setting the array" (verified, F-TR-7).
    fn foreach_set(&mut self, prop: &'static PropDef, data: &Bulk) -> Result<(), RnaError> { … }
}
pub enum SetOutcome { Done, NeedsHook }                         // NeedsHook => §9.5
```

`Borrowed` costs nothing for in-memory structs (modifiers, nodes, plain settings); proxies are a pointer and an index. These signatures are exercised in M0 by `MeshVertexMut`, the derived `MaterialSlot`, an embedded node-tree path and `Context` before they freeze (§22.2). Most bindings are one line per property through `rna_struct!` (which also generates proxy types for `soa(...)` bindings). Generated plain structs (§4.2) need no hand-written binding.

### 9.5 Hooks and RNA functions (global tier)

Anything touching more than its own struct registers globally, one file per hook: RNA functions (`new`, `remove`, `link`, `keyframe_insert`, `select_set`, `frame_set`, `from_pydata`, `foreach_get`, `foreach_set`, `vertex_groups.new`, `VertexGroup.add`, `nodes.new`, `links.new`, `evaluated_get`, `to_mesh`, `path_resolve`, …) and setters with side effects (`rotation_mode` conversion, `parent`, `name` uniqueness, `light.type` re-typing, `dimensions`, `matrix_world`, `frame_current` (marks time dirty only, §7.3), `MaterialSlot.link`/`material`).

```rust
// frap-scene/src/host.rs (CONTRACT)
pub trait HostRead {
    fn main(&self) -> &Main; fn win(&self) -> &WindowState; fn edit(&self) -> &EditState;
    fn ctx(&self) -> Context<'_>;
    /// Read-only, cached view of a mesh in Edit Mode (§6.3); the original Mesh otherwise.
    fn mesh_view(&self, mesh: IdRef<MeshId>) -> Arc<Mesh>;
}
pub trait Host: HostRead {     // implemented by frap_session::HostImpl, a field of Session disjoint from
                               // its Services, so an OpCtx can borrow both (F-SES-1)
    fn main_mut(&mut self) -> (&mut Main, &mut ChangeLog);
    fn edit_mut(&mut self) -> &mut EditState;
    fn split_edit(&mut self) -> (Context<'_>, &mut EditState);   // disjoint fields of the state (F-OP-5)
    fn win_mut(&mut self) -> &mut WindowState;
    fn eval(&mut self) -> (&dyn HostRead, &mut dyn Evaluator);   // split-borrow helper
    /// COW clone of the state with pending animation applied at `frame` (scene frame if None) and a
    /// fresh evaluator, for render and export (§7.3, §19.4; F-OP-7).
    fn snapshot_at(&mut self, frame: Option<i32>) -> Result<Snapshot, RnaError>;
    fn mark_time_dirty(&mut self);                         // frame_current setter
    fn apply_pending_animation(&mut self) -> Result<(), RnaError>;  // evaluation boundaries (§7.3)
    fn report(&mut self, r: Report);
    fn set_path(&mut self, owner: Root, data_path: &str, index: i32, v: Value) -> Result<(), RnaError>; // animation; owner is Id or Embedded
}
pub struct Snapshot { pub host: Box<dyn HostRead + Send>, pub eval: Box<dyn Evaluator + Send> }
pub type RnaFuncImpl = fn(&mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError>;
pub type RnaSetHook  = fn(&mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError>;
/// Reads need no transaction (§12), so get hooks take &dyn HostRead and may return a proxy that
/// borrows the scene (material_slots → CollRef::Owned(MaterialSlots { main, ob })) (F-HOOK-1).
pub type RnaGetHook  = for<'h> fn(&'h dyn HostRead, this: &RnaPtr, prop: &'static PropDef) -> Result<Field<'h>, RnaError>;

pub struct RnaFuncReg { pub srna: &'static str, pub func: &'static str, pub imp: RnaFuncImpl }
pub struct RnaHookReg { pub srna: &'static str, pub prop: &'static str,
                        pub get: Option<RnaGetHook>, pub set: Option<RnaSetHook> }   // lookup walks the base chain
pub struct FuncArgs { pub func: &'static FuncDef, pub values: IndexMap<&'static str, Value>, pub is_set: BitSet }
#[linkme::distributed_slice] pub static RNA_FUNCS: [RnaFuncReg];
#[linkme::distributed_slice] pub static RNA_HOOKS: [RnaHookReg];   // get and/or set per (srna, prop)
```

`FuncArgs` binds positional and keyword arguments against `FuncDef.params`, enforces keyword-only markers (`obj.keyframe_insert("location", 2, 20)` → Blender's TypeError) and coerces each argument. Python-only members (`children`, `children_recursive`, `users_collection`, `users_scene`, `Mesh.from_pydata/shade_smooth/shade_flat`, `bpy_struct.keyframe_insert/keyframe_delete` signatures) come from `spec/overlay/python_only.toml` and are implemented as hooks flagged `PYTHON_ONLY`. An RNA function or hook is registered in a crate that can name `frap_scene::{RNA_FUNCS, RNA_HOOKS}` (L6 or above): crates below L6 (`frap-mesh`, `frap-nodes`) provide the Rust API and the struct-local bindings, and the global registration lives in `frap-scene/src/{funcs,hooks}/` or `frap-ops-data/src/funcs/` (e.g. `Mesh.from_pydata`, decision log W0-22).

### 9.6 Worked example: the property `Object.location`

**1. Dump** (`spec/blender-5.2/rna/Object.json`, excerpt; values verified in Appendix A):

```json
{"identifier":"Object","base":"ID","name_property":"name","props":[
  {"identifier":"location","name":"Location","type":"FLOAT","subtype":"TRANSLATION","unit":"LENGTH",
   "array":[3],"default":[0,0,0],"hard_min":-3.4028235e38,"hard_max":3.4028235e38,
   "soft_min":-3.4028235e38,"soft_max":3.4028235e38,"precision":5,"flags":["ANIMATABLE"]}, "…"]}
```

**2. Generated** (`crates/frap-spec-rna/src/generated/object.rs`, never edited):

```rust
pub static OBJECT: StructDef = StructDef { identifier: "Object", base: Some(&ID), name_property: Some("name"),
    props: &OBJECT_PROPS, funcs: &OBJECT_FUNCS, is_id: true, /* … */ };
pub mod object { pub const LOCATION: &PropDef = &super::OBJECT_PROPS[17]; /* one const per prop (F-GEN-1) */ }
pub static OBJECT_PROPS: [PropDef; N] = [ /* … */
    PropDef { identifier: "location", name: "Location", description: "Location of the object",
        kind: PropKind::Float { default: Defaults::Array(&[0.0, 0.0, 0.0]), hard: (-3.4028235e38, 3.4028235e38),
                                soft: (-3.4028235e38, 3.4028235e38), precision: 5, storage: FloatStorage::F32 },
        subtype: Subtype::Translation, unit: Unit::Length, array: ArrayShape::Fixed(3),
        flags: PropFlags::ANIMATABLE, deprecated: None },
    /* … */ ];
```

**3. Data field** (`crates/frap-scene/src/id/object/mod.rs`): `pub location: [f32; 3]`.

**4. Binding** (`crates/frap-scene/src/id/object/rna.rs`, hand-written, one line per property):

```rust
rna_struct! { Object as frap_spec_rna::OBJECT {
    name            => hook,                                   // unique_name + re-sort (RNA_HOOKS)
    location        => location (tag = Vector),                // [f32;3] <-> Value::FloatArray{tag: Vector}
    rotation_euler  => rotation_euler (tag = Euler(rotation_mode)),
    rotation_mode   => hook,                                   // converts stored rotation
    scale           => scale (tag = Vector),
    data            => id(data),
    parent          => hook,                                   // RNA assign: matrix_parent_inverse = identity
    matrix_world    => hook,                                   // evaluated read; decomposing write
    dimensions      => hook,                                   // evaluated bounds
    type            => readonly(object_type()),
    modifiers       => coll(modifiers, api = "ObjectModifiers"),
    material_slots  => hook_coll,                              // derived from data.materials + overrides (§5.6)
    vertex_groups   => hook_coll,                              // names live on the Mesh
    children        => python_only hook,
}}
#[cfg(test)] #[test] fn bindings() { frap_rna::testing::check_bindings::<Object>(); }
```

The macro generates `impl Rna`/`impl RnaMut` with a `match prop.identifier` that converts `[f32; 3]` to and from `Value::FloatArray { shape: Fixed(3), tag: Vector }`.

**5. Set at runtime**: `bpy.data.objects['Cube'].location = (1, 2, 3)`
1. The executor resolves `bpy.data.objects['Cube']` → `RnaPtr { root: Id(OB Cube) }`, then `.location` → `Resolved::Prop { prop: object::LOCATION }` (`frap_spec_rna::object::LOCATION`).
2. `coerce(object::LOCATION, Input::Tuple([Int(1), Int(2), Int(3)]), &CoerceCx::RnaSet { owner: "Object" })` → `Value::FloatArray([1.0, 2.0, 3.0], Fixed(3), Vector)`.
3. The transaction walks again **mutably** from the owning ID: `main.objects.get_mut(cube, log)` (COW, `generation` bump, `modified += OB Cube`) → `RnaMut::set` → `SetOutcome::Done`.
4. Response: `{"kind":"set","ok":true,"path":"bpy.data.objects['Cube'].location","value":[1,2,3],"changes":{"modified":["bpy.data.objects['Cube']"]}, …}`.

Component writes (`….location.x = 5`, `….location[1] = 7`) resolve with `component`/`index`, read the array, replace one element, and write back through the same path. `….location = (1, 2)` → `ValueError: bpy_struct: item.attr = val: sequences of dimension 0 should contain 3 items, not 2` (verified wording).

**6. Get**: `bpy.data.objects['Cube'].location` → `{"kind":"get","value":[1,2,3],"rna":{"type":"FLOAT","subtype":"TRANSLATION","array":[3],"math":"Vector"}}`.

---

## 10. Operator registry

### 10.1 Definitions

```rust
// frap-rna/src/op.rs (CONTRACT): shared by generated code and the framework
#[derive(Clone, Debug)]
pub struct Prop<T> { pub value: T, pub is_set: bool }      // RNA_struct_property_is_set
impl<T> Prop<T> { pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Prop<U>; }   // F-OP-4
pub struct BitSet(SmallVec<[u64; 2]>);                      // F-OP-1 (glTF export has 110 props)
pub struct OpArgs {                                         // output of argument conversion
    pub values: IndexMap<&'static str, Value>,              // spec order, defaults filled for unset props
    pub is_set: BitSet,                                     // indices into `values`
    pub nested: IndexMap<&'static str, OpArgs>,             // macro sub-ops: "TRANSFORM_OT_translate"
}
impl OpArgs { pub fn prop<T: FromValue>(&self, index: usize, identifier: &'static str) -> Prop<T>; }  // F-OP-2
pub trait FromValue: Sized { fn from_value(v: &Value) -> Self; }   // infallible after coerce; bool, i32, f32,
                                                            // SmolStr, [f32; N], [[f32; C]; R], [bool; N], [i32; N],
                                                            // BTreeSet<SmolStr>, Vec<IndexMap<..>>, generated enums
pub trait OpProps: Sized {
    const IDNAME: &'static str;                             // "mesh.primitive_cube_add"
    const BL_IDNAME: &'static str;                          // "MESH_OT_primitive_cube_add"
    fn spec() -> &'static OperatorSpec;
    fn from_args(a: &OpArgs) -> Self;                       // generated field-by-field extraction
}

// frap-rna/src/op.rs; statics generated into frap-spec-ops-<group> or decoded by frap-spec-catalog
pub struct OperatorSpec {
    pub idname: &'static str, pub bl_idname: &'static str, pub label: &'static str,
    pub description: &'static str, pub options: OpOptions,  // REGISTER | UNDO | INTERNAL | MACRO | …
    pub props: &'static [PropDef],
    pub macro_steps: &'static [&'static str],               // ["MESH_OT_extrude_region", "TRANSFORM_OT_translate"]
    pub poll_matrix: Option<PollMatrix>,                    // dumped A–E (+V) results; None for hand-written frap.* (F-OP-3)
}
pub struct PollMatrix { pub plain: [PollCell; 5], pub view3d: [PollCell; 5] }
pub enum PollCell { Y, N, Raised }

// frap-op/src/def.rs (CONTRACT)
pub struct OperatorDef {
    pub spec: &'static OperatorSpec,
    pub poll: Poll,                                         // declarative requirements + check (§8)
    pub exec: fn(&mut OpCtx, &OpArgs) -> OpResult,          // #[operator] wraps the typed fn
    pub kind: OpKind,                                       // Normal | Macro
    pub dyn_enums: &'static [(&'static str, DynEnumFn)],    // mode_set.mode, modifier_add.type, merge.type
    pub tier: Tier,                                         // T0..T4 (§2.1)
    pub support: Support,                                   // Full | Partial(&str) | HeadlessExtended(&str)
    pub notes: &'static [&'static str],                     // frap_note for --describe
}
#[linkme::distributed_slice] pub static OPERATORS: [OperatorDef];
pub type DynEnumFn = fn(&Context) -> Vec<EnumItemOwned>;
pub struct Poll { pub requires: &'static [PollReq], pub check: fn(&Context) -> Result<(), PollFail> }
pub struct PollFail { pub message: Option<String>,          // None => "failed, context is incorrect"
                      pub unmet: SmallVec<[PollReq; 2]>, pub hint: Option<String> }

pub enum OpStatus { Finished, Cancelled }
pub enum OpError { Runtime(String) /* raised as RuntimeError("Error: <msg>") */, Rna(RnaError),
                   NotImplemented(String), Io(String), Gpu(String), Limit(String) }   // From<RnaError>
pub type OpResult = Result<OpStatus, OpError>;
// Report { level: ReportType, message } and ReportType { Info, Warning, Error, Operator, Property } live in
// frap-rna/src/error.rs (frap-scene and frap-material name them; F-LAYER-1) and are re-exported by frap-op.

/// Side-effect services that live outside the snapshot (never rolled back). Implemented by frap-session.
pub trait Services {
    fn render(&mut self) -> Result<&mut dyn RenderService, OpError>;   // lazy GPU device; GPU_UNAVAILABLE if none
    fn images(&mut self) -> &mut dyn ImageCache;                      // decoded pixels by (IdKey, gen)
    fn paths(&self) -> &PathResolver;                                 // cwd, `//` relative to bpy.data.filepath
}
pub struct PathResolver { pub cwd: PathBuf, pub blend_dir: Option<PathBuf> }   // F-REN-1
pub trait ImageCache {                                                // F-REN-1
    fn get(&self, image: IdRef<ImageId>, generation: u64) -> Option<Arc<DecodedImage>>;
    fn put(&mut self, image: IdRef<ImageId>, generation: u64, pixels: Arc<DecodedImage>);
    fn evict(&mut self, key: IdKey);
}

pub struct OpCtx<'h> { host: &'h mut dyn Host, services: &'h mut dyn Services, reports: Vec<Report>,
                       result: IndexMap<SmolStr, Value>, op: &'static OperatorSpec }
impl<'h> OpCtx<'h> {
    pub fn new(host: &'h mut dyn Host, services: &'h mut dyn Services, op: &'static OperatorSpec) -> Self;  // F-OP-8
    pub fn finish(self) -> OpOutput;                        // OpOutput { reports, result } (F-OP-8)
    pub fn ctx(&self) -> Context<'_>;
    pub fn main(&self) -> &Main;
    pub fn main_mut(&mut self) -> (&mut Main, &mut ChangeLog);   // inserts into any IdMap (F-OP-9)
    pub fn object_mut(&mut self, r: IdRef<ObjectId>) -> Result<&mut Object, OpError>;
    pub fn mesh_mut(&mut self, r: IdRef<MeshId>) -> Result<&mut Mesh, OpError>;
    pub fn bmesh_mut(&mut self, me: IdRef<MeshId>) -> Result<&mut BMesh, OpError>;   // EditState COW, bumps bm_gen
    /// Read the context (tool settings, cursor, selection) while editing BMeshes.
    pub fn split_edit(&mut self) -> (Context<'_>, &mut EditState);
    /// Read the state while asking the evaluator for evaluated objects; the context is
    /// HostRead::ctx() (a Context is not a HostRead, F-OP-6).
    pub fn split_eval(&mut self) -> (&dyn HostRead, &mut dyn Evaluator);
    pub fn services(&mut self) -> &mut dyn Services;
    pub fn snapshot_at(&mut self, frame: Option<i32>) -> Result<Snapshot, OpError>;   // render/export (F-OP-7)
    pub fn add_object(&mut self, name: &str, data: Option<IdAny>, at: Placement) -> IdRef<ObjectId>; // link to ctx.collection, select only it, make active
    pub fn evaluated(&mut self, ob: IdRef<ObjectId>) -> Result<Arc<EvaluatedObject>, OpError>;
    pub fn report(&mut self, level: ReportType, msg: impl Into<String>);
    pub fn result(&mut self, key: &str, v: impl Into<Value>);   // Response.result; IdRef<K>: Into<Value>
    pub fn run_op(&mut self, idname: &str, args: OpArgs) -> OpResult;   // ops calling ops
}
```

Registry: `Registry::global()` is built once from `OPERATORS` plus `frap_spec_catalog::catalog()`. Building it **fails** (panic at startup, caught by the registry test in CI) if two `OperatorDef`s share an idname or if an implemented operator has no typed spec. `lookup(idname) -> Lookup::{Implemented(&OperatorDef), KnownNotImplemented(&OperatorSpec), Unknown{suggestions}}`.

### 10.2 Invocation pipeline (identical for every front-end)

`Session::invoke(idname, Inputs { positional, kwargs })`:

1. **Lookup.** Unknown → `AttributeError: Calling operator "bpy.ops.mesh.foo" error, could not be found` (verified) + `did_you_mean`. Known but unimplemented → `NotImplementedError`, code `OP_NOT_IMPLEMENTED`, with the Blender signature in `detail`.
2. **Positional arguments.** A string equal to a Blender execution context (`'EXEC_DEFAULT'`, `'INVOKE_DEFAULT'`, …) is accepted and ignored (frap always executes; INVOKE adds an INFO report). Anything else → `TypeError`, code `OP_ARG_POSITIONAL` (deviation: Blender silently ignores it, Appendix A).
3. **Convert keyword arguments** against `spec.props`. Unknown keyword → `TypeError: Converting py args to operator properties:: keyword "sizee" unrecognized` (double colon, verified) + suggestions + valid list. Wrong type → `TypeError: Converting py args to operator properties: MESH_OT_primitive_cube_add.size expected a float type, not str` (verified). Bad enum → `TypeError: Converting py args to operator properties: enum "FOO" not found in ('WORLD', 'VIEW', 'CURSOR')` (verified); dynamic enums are validated against `DynEnumFn(ctx)`. Wrong length → `ValueError`. Out of range → clamped + INFO report. Macro sub-dicts (`TRANSFORM_OT_translate={"value": (0,0,1)}` or short-form `TRANSFORM_OT_translate.value=0,0,1`) recurse into `OpArgs.nested`. All of this happens **before** poll, as in Blender.
4. **Ensure evaluated** (Blender evaluates the depsgraph before exec), including pending animation after a `frame_current` change (§7.3).
5. **Poll** → `RuntimeError: Operator bpy.ops.X.poll() failed, context is incorrect` or the custom message, with `detail.requires`, `detail.unmet` and a `hint` naming the fix (§8).
6. **Begin transaction** (§12).
7. **Exec.** A macro runs its steps in order with their nested args; a cancelled step cancels the macro.
8. **Outcome.** `Finished` commits. `Cancelled` rolls back; `status:"CANCELLED"` with Blender's WARNING reports, or a synthesized one where Blender is silent [OBJ §1.5]. `Err` rolls back; `status:"ERROR"`.
9. **Build the response** (§13): reports, `result`, `changes` from the `ChangeLog`, `context`, `edit_mesh`.

The numbering names the steps; they run in Blender's order 1, 2, 5, 3 (`pyop_call` polls before `pyrna_pydict_to_props`; verified: `mesh.merge(type='NOPE')` or `mesh.merge(typo='NOPE')` in Object Mode raise the poll `RuntimeError`), then 4, 6–9. Keyword conversion sees the context, so a dynamic enum is checked against its items in the current context and its `TypeError` lists them (`mode_set(mode='EDIT')` on a camera: `not found in ('OBJECT')`). An operator that re-checks a dynamic enum in exec (items that depend on edit-mode state, e.g. `mesh.merge` `FIRST`/`LAST`) raises the same `TypeError` with code `OP_ARG_ENUM` (decision log M2S1-1, D-78).

### 10.3 Is-set semantics and shared helpers (`frap-op/src/helpers/`)

- `placement(cx, &location, &rotation, align: Align, &scale) -> Result<Placement, OpError>`: unset `location` → 3D cursor; unset `rotation` → from `align` (`WORLD` identity, `VIEW` virtual view rotation, `CURSOR` cursor rotation); `scale` sentinel `(0,0,0)` → 1 [OBJ §1.3]. `align` is taken by value: Blender reads only its value, never whether it was set (decision log W0-25).
- `add_primitive_or_merge(cx, name, mesh, placement, enter_editmode)`: in object mode creates an object and a mesh, both requested as `name` and **uniquified independently** in `objects` and `meshes` (after deleting the factory Cube: object `Cube`, mesh `Cube.001`, verified, §5.4), links it to `context.collection`, deselects others, selects and activates it; in edit mode merges into the edit mesh and selects only the new geometry. `add_object_with_data` applies the same rule for cameras, lights, empties and armatures.
- `add_object_with_data(cx, name, data: Option<IdAny>, placement, enter_editmode) -> Result<IdRef<ObjectId>, OpError>` (`data` inserted by the caller under its own requested name; `None` = Empty), `frame_range_or_scene(cx, &frame_start, &frame_end) -> (i32, i32)`, `iter_edit_meshes(cx) -> Vec<(IdRef<ObjectId>, IdRef<MeshId>)>` (multi-object edit loop). Signatures frozen at M1 W0; bodies WP 1.12.
- **Transform dispatch is an extension point.** `transform.translate/rotate/resize` (`frap-ops-transform/src/ops/*.rs`, owned once in M1) call the shared core and dispatch per context mode through `TRANSFORM_MODE_HANDLERS`: `object/` (M1), `edit/` (M2), `pose/` (M6). Later milestones add handler files only.

```rust
// frap-op/src/modes.rs (CONTRACT, M1 W0)
pub struct ModeHandler { pub mode: ObjectMode, pub object_type: &'static str /* "MESH", "" = any */,
                         pub enter: fn(&mut OpCtx, objects: &[IdRef<ObjectId>]) -> OpResult,
                         pub exit:  fn(&mut OpCtx, objects: &[IdRef<ObjectId>]) -> OpResult }
pub enum TransformOp { Translate, Rotate, Resize }
pub struct TransformModeHandler { pub mode: ContextMode, pub apply: fn(&mut OpCtx, op: TransformOp, args: &OpArgs) -> OpResult }
#[linkme::distributed_slice] pub static MODE_HANDLERS: [ModeHandler];
#[linkme::distributed_slice] pub static TRANSFORM_MODE_HANDLERS: [TransformModeHandler];
```

### 10.4 Worked example: `mesh.primitive_cube_add`

**1. Dump** (`spec/blender-5.2/ops/mesh.json`, excerpt; every value verified, Appendix A):

```json
{"idname":"mesh.primitive_cube_add","bl_idname":"MESH_OT_primitive_cube_add","label":"Add Cube",
 "description":"Construct a cube mesh that consists of six square faces","options":["REGISTER","UNDO"],
 "poll":{"A":"Y","B":"Y","C":"Y","D":"Y","E":"Y"},
 "props":[
  {"identifier":"size","type":"FLOAT","subtype":"DISTANCE","unit":"LENGTH","default":2.0,
   "hard_min":0.0,"hard_max":999999995904.0,"soft_min":0.001,"soft_max":100.0},
  {"identifier":"calc_uvs","type":"BOOLEAN","default":true},
  {"identifier":"enter_editmode","type":"BOOLEAN","default":false,"flags":["HIDDEN","SKIP_SAVE"]},
  {"identifier":"align","type":"ENUM","items":["WORLD","VIEW","CURSOR"],"default":"WORLD"},
  {"identifier":"location","type":"FLOAT","subtype":"XYZ_LENGTH","unit":"LENGTH","array":[3],"default":[0,0,0],"flags":["SKIP_SAVE"]},
  {"identifier":"rotation","type":"FLOAT","subtype":"EULER","unit":"ROTATION","array":[3],"default":[0,0,0],"flags":["SKIP_SAVE"]},
  {"identifier":"scale","type":"FLOAT","subtype":"XYZ_LENGTH","unit":"LENGTH","array":[3],"default":[0,0,0],"flags":["HIDDEN","SKIP_SAVE"]}]}
```

**2. Generated** (`crates/frap-spec-ops-mesh/src/generated/mesh.rs`, never edited):

```rust
pub mod primitive_cube_add {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Align { World, View, Cursor }               // "WORLD" | "VIEW" | "CURSOR"
}
#[derive(Debug, Clone)]
pub struct PrimitiveCubeAdd {
    pub size: Prop<f32>,                                 // default 2.0, hard [0, 1e12], DISTANCE
    pub calc_uvs: Prop<bool>,                            // default true
    pub enter_editmode: Prop<bool>,                      // HIDDEN, SKIP_SAVE
    pub align: Prop<primitive_cube_add::Align>,
    pub location: Prop<[f32; 3]>,                        // unset => 3D cursor
    pub rotation: Prop<[f32; 3]>,                        // unset => from align
    pub scale: Prop<[f32; 3]>,                           // HIDDEN; (0,0,0) => unset
}
impl OpProps for PrimitiveCubeAdd {
    const IDNAME: &'static str = "mesh.primitive_cube_add";
    const BL_IDNAME: &'static str = "MESH_OT_primitive_cube_add";
    fn spec() -> &'static OperatorSpec { &OPSPEC_MESH_OT_PRIMITIVE_CUBE_ADD }
    fn from_args(a: &OpArgs) -> Self { PrimitiveCubeAdd { size: a.prop(0, "size"), /* … */ scale: a.prop(6, "scale") } }
}
```

**3. Geometry** (`crates/frap-mesh/src/primitives/cube.rs`): `pub fn cube(size: f32, calc_uvs: bool) -> Mesh` reproducing [MESH §1.4] exactly (verts `0(-1,-1,-1) 1(-1,-1,1) …`, edges `[2,0][0,1]…`, faces `0:[0,1,3,2] … 5:[7,3,1,5]`, the UV cross layout, `sharp_face` all true).

**4. Operator** (`crates/frap-ops-mesh/src/ops/primitive_cube_add.rs`: the whole unit of work, one agent, one file):

```rust
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::PrimitiveCubeAdd as P;

#[operator(poll = poll::always, tier = T1)]
fn exec(cx: &mut OpCtx, p: P) -> OpResult {
    let at = helpers::placement(cx, &p.location, &p.rotation, &p.align.map(Into::into), &p.scale)?;
    let mesh = frap_mesh::primitives::cube(p.size.value, p.calc_uvs.value);
    let ob = helpers::add_primitive_or_merge(cx, "Cube", mesh, at, p.enter_editmode.value)?;
    cx.result("object", ob);                 // {"$ref": "bpy.data.objects['Cube.001']"}
    Ok(OpStatus::Finished)
}
```

`#[operator]` checks the function takes the generated props type, emits an `OperatorDef` into `OPERATORS` (idname from `P::IDNAME`), wraps `exec` with `P::from_args`, and generates a `#[test]` that the spec exists. (`add_primitive_or_merge` applies the placement's scale to positions, so object scale stays 1, as in Blender.)

**5. Runtime**: `Registry::global().lookup("mesh.primitive_cube_add")` → `Implemented`. `--describe op:mesh.primitive_cube_add` shows the dumped props plus `"frap":{"status":"implemented","tier":"T1","unset_semantics":{"location":"3D cursor","rotation":"from align","scale":"(0,0,0) means 1"}}`.

**6. Golden** (`tests/golden/cases/primitives/cube_add_location.py`):

```python
# golden: startup=factory compare=objects,mesh:Cube.001 order=exact tol=1e-5
import bpy
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))
```

Verified Blender result (Appendix A): object `Cube.001`, mesh `Cube.001`, selection `['Cube.001']`, location `(0,0,1)`, 8 vertices, `bpy.data.objects` order `['Camera','Cube','Cube.001','Light']`.

### 10.5 Self-check

`frap-ops-all::link()` references one symbol per ops crate so the linker keeps every `linkme` entry. A `linkme` slice is a link-time static, so a crate the linker drops would vanish from any second slice too; the expected set must therefore come from outside the binary. `crates/frap-ops-all/tests/registry.rs`:

1. **Expected set from the source tree.** The test walks `crates/frap-ops-*/src/ops/*.rs` and `src/funcs/*.rs` (via `CARGO_MANIFEST_DIR`) and maps file names to idnames and `(srna, func)` pairs with the §21.1 naming rule (`ops/<op>.rs` in the crate's primary namespace, `ops/<ns>__<op>.rs` otherwise). Nothing is committed, so no shared file is edited by parallel WPs.
2. **Compared with the runtime registry**: every expected operator and RNA function is registered, and nothing is registered that has no file.
3. **Uniqueness**: no duplicate operator idname, `(srna, func)` or `(srna, prop)` hook; `Registry::global()` itself refuses duplicates.
4. **Hook completeness**: every `=> hook` binding (`HOOK_DECLS`, §4.3) has exactly one `RNA_HOOKS` entry.

The test runs in every PR on all three OSes, with the release profile too (LTO is where entries get dropped).

---

## 11. Front-ends: bpy-line and short form

### 11.1 Pipeline

```
raw stdin bytes ─► line splitter (bracket continuation, ';', comments) ─┐
serve/MCP request {"id","src"} ─► parse_complete(src) (never waits) ─────┴─► per statement:
   form detection ─► bpy-line parser | short-form parser ─► Stmt { cmd: Command, … }   (frap-lang, no scene)
   ─► Session::exec(&Stmt) ─► evaluate Exprs to Input ─► invoke / get / set / call ─► Response   (frap-session)
```

`frap_lang::parse_complete(src) -> Vec<Result<Stmt, FrapError>>` treats `src` as a complete unit: an unclosed bracket or string is an immediate `SyntaxError: '(' was never closed`, never a request for more input. The raw-stdin splitter is the only place that continues across lines; it also ends a pending continuation with that `SyntaxError` on a blank line or at EOF, so a human session cannot hang on a typo either.

### 11.2 IR (`frap-lang/src/ir.rs`, CONTRACT)

```rust
pub struct Stmt { pub cmd: Command, pub span: Span, pub line: u32, pub form: Form /* Bpy | Short */ }
/// Span (defined in frap-rna/src/error.rs, re-exported): `start`/`len` are UTF-8 **byte** offsets into the statement source; `line` and `col` are 1-based,
/// `col` counts bytes and `col_chars` counts Unicode code points (both are reported in JSON, §13.4).
pub struct Span { pub start: u32, pub len: u32, pub line: u32, pub col: u32, pub col_chars: u32 }
pub enum Command {
    Invoke { idname: SmolStr, positional: Vec<Expr>, args: Vec<Arg> },   // bpy.ops.ns.op(...) | ns.op k=v
    OpMeta { idname: SmolStr, what: OpMetaKind },                        // bpy.ops.ns.op.poll() | .get_rna_type() | .idname() | .idname_py()
    Get    { target: PathExpr },                                         // bare path
    Set    { target: PathExpr, op: AssignOp, value: Expr },              // = += -= *= /=
    Call   { target: PathExpr, args: Vec<Arg> },                         // path ending in an RNA function
    Print  { values: Vec<Expr> },                                        // print(a, b)
    Noop   { reason: NoopReason },                                       // import bpy | from mathutils import … | pass | blank
}
pub struct Arg { pub name: Option<SmolStr>, pub value: Expr, pub span: Span }
pub enum Expr {
    Lit(Lit, Span),                          // Int, Float, Str, Bool, None
    Bare(SmolStr, Span),                     // short form only: unquoted word (enum id, name)
    Path(PathExpr),                          // read, or nested call when the path ends with Call
    Tuple(Vec<Expr>, Span), List(Vec<Expr>, Span), Set(Vec<Expr>, Span), Dict(Vec<(Expr, Expr)>, Span),
    Unary(UnOp, Box<Expr>, Span), Binary(BinOp, Box<Expr>, Box<Expr>, Span),   // + - * / @ **
    Builtin { name: Builtin, args: Vec<Arg>, span: Span },   // Vector Euler Quaternion Matrix Color math.* radians degrees float int
    Method { recv: Box<Expr>, name: SmolStr, args: Vec<Arg>, span: Span },     // Vector(...).normalized()
}
pub struct PathExpr { pub root: PathRoot, pub segs: Vec<PathSeg>, pub span: Span }
pub enum PathRoot { BpyData, BpyContext }    // bpy.data | D | data ; bpy.context | C | context
pub enum PathSeg { Attr(SmolStr), Key(String), KeyLib(String, Option<String>), Index(i64), Slice(Option<i64>, Option<i64>), Call(Vec<Arg>) }
// KeyLib: the (name, library) tuple key, bpy.data.node_groups['Smooth by Angle', None] (§5.2); WP 1.10/1.14 resolve it.
```

The IR knows nothing about operators or RNA. Calls with side effects inside a statement (`bpy.data.materials.new("Red")`) and nested ones (`mesh.materials.append(bpy.data.materials.new("Red"))`) are evaluated innermost first inside the statement's transaction, which removes most of the need for variables.

### 11.3 bpy-line form (a Python statement subset)

- Statements separated by newline or `;` (outside strings). In raw-stdin mode a statement continues while brackets are open (§11.1); in `serve`/MCP mode each request is complete. `#` starts a comment outside strings.
- Allowed: `bpy.ops.<ns>.<op>(…)`; `bpy.ops.<ns>.<op>.poll()`, `.get_rna_type()`, `.idname()`, `.idname_py()`; path assignment and augmented assignment; path function calls; bare path expressions (read); `print(…)`; `import bpy|math|mathutils|bmesh`, `from mathutils import …`, `from math import …` (no-ops). `D`/`C` are aliases of `bpy.data`/`bpy.context`.
- Literals: Python ints/floats (`_`, exponents, `float('inf')`), strings `'…'`, `"…"`, `r'…'` with escapes (no f-strings), `True/False/None`, tuples (incl. `(x,)`), lists, sets (`{'VERT'}`, `set()`), dicts (macro sub-props).
- Expressions: unary `-`/`+`, `+ - * / ** @`, mathutils constructors and their common methods (`normalized`, `to_quaternion`, `to_euler`, `to_matrix`, `Matrix.Translation/Rotation/Scale/Identity`), `math.pi/radians/degrees/sqrt/sin/cos/tan`. Euler matrices use Blender's `eul_to_mat3`, never `glam::EulerRot` [RUST §2].
- A lone identifier or an unknown call (`x`, `cube_add()`) is CPython's `NameError`, code `NAME`. A bare expression that is not a path (`path * 2`, literals, `(…)`, `True`) is `UNSUPPORTED_SYNTAX` with a hint to wrap it in `print(...)` (decision log W1-7).
- Rejected with `SyntaxError`, code `UNSUPPORTED_SYNTAX`: binding a bare name (`obj = …` → hint `use a full path such as bpy.data.objects['Cube']`), `for/while/if/def/class/with/lambda`, comprehensions, f-strings. Python's own wording is kept for real syntax errors (`'(' was never closed`).

### 11.4 Short form

```
stmt     := op_call | path_stmt
op_call  := NS '.' OPNAME (WS kwarg)*                  # mesh.primitive_cube_add size=2 location=0,0,1
kwarg    := key '=' svalue
key      := IDENT ('.' IDENT)?                         # TRANSFORM_OT_translate.value=0,0,1 (macro sub-op)
path_stmt:= spath                                      # get
          | spath WS? ('='|'+='|'-='|'*='|'/=') WS? svalue   # set
          | spath (WS (kwarg | svalue))+               # call: data.objects['Cube'].select_set True
          | spath '(' args ')'                         # call, Python-style
spath    := ROOT ('.' IDENT | '[' key ']') accessor*  # a path needs a root followed by '.' or '['
ROOT     := 'data' | 'context' | 'D' | 'C'
svalue   := item (',' item)*                           # comma tuple, no spaces: 0,0,1
          | '(' bpy-expr ')' | '[' bpy-expr ']'        # escape hatch: full expression grammar
item     := NUMBER | true|false|True|False|none|None | QUOTED | '{' item (',' item)* '}' | spath | BARE
BARE     := [A-Za-z_][A-Za-z0-9_.\-]*                  # any word that is not an spath -> Expr::Bare: enum id or name
```

A word is a path **only** if it starts with a `ROOT` immediately followed by `.` or `[`; otherwise it is `BARE`. So `name=data` passes the string `data`, and `data.objects['Cube']` is a path (the `BARE` rule never swallows the `[`).

**Form detection** (per statement, purely lexical, no operator list needed in `frap-lang`):
1. First token `bpy`, `print`, `import`, `from` → bpy-line.
2. First token `D` or `C`: look at what follows the full path. `=`, `+=`, …, `(`, end of statement → bpy-line; whitespace followed by a value or `key=value` → short form (`D.objects['Cube'].select_set True` is a short call).
3. First token `data`/`context` followed by `.` or `[` → short path statement.
4. Any other `IDENT '.' IDENT` → short op call. An unknown namespace or operator is reported later by the registry as `OP_UNKNOWN` with suggestions, not as a syntax error.
5. Anything else → `SyntaxError` with a hint listing both forms.

The forms may be mixed freely in one stream. Short-form numbers lex as `Lit::Int`/`Lit::Float` and quoted strings as `Lit::Str`, so typed values lower to exactly the same IR as bpy-line; only unquoted words become `Expr::Bare`. Positional values are allowed only for RNA function calls (mapped in `FuncDef` order); operators are keyword-only.

### 11.5 Worked example: one call, two syntaxes

```
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))
mesh.primitive_cube_add size=2 location=0,0,1
```

Both lower to the same `Command` (spans aside), asserted by an `insta` corpus test (`crates/frap-lang/tests/corpus/primitive_cube_add.txt`):

```rust
Command::Invoke {
    idname: "mesh.primitive_cube_add".into(),
    positional: vec![],
    args: vec![
        Arg { name: Some("size".into()),     value: Expr::Lit(Lit::Int(2), _) },
        Arg { name: Some("location".into()), value: Expr::Tuple(vec![Expr::Lit(Lit::Int(0), _),
                                                                     Expr::Lit(Lit::Int(0), _),
                                                                     Expr::Lit(Lit::Int(1), _)], _) },
    ],
}
```

The executor evaluates each `Expr` to `Input` (`Int(2)`, `Tuple([Int(0), Int(0), Int(1)])`) and calls `Session::invoke`; `coerce` makes `size = 2.0f32` and `location = [0.0, 0.0, 1.0]`, both with `is_set = true`. Likewise:

```
bpy.data.objects['Cube'].location = (1, 2, 3)        data.objects['Cube'].location = 1,2,3
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, 1)})
mesh.extrude_region_move TRANSFORM_OT_translate.value=0,0,1
bpy.ops.mesh.select_mode(type='FACE')                mesh.select_mode type=FACE      # Lit::Str vs Bare: same Value::Enum after coerce
```

### 11.6 Future front-ends

`Session` exposes the same entry points the executor uses, free of parser types:

```rust
impl Session {
    pub fn exec_request(&mut self, req: Request) -> Vec<Response>;   // serve/MCP framing (§13.1); the caller
                                                                     // then writes DoneRecord::from_responses(id, &rs)
    pub fn invoke(&mut self, idname: &str, kwargs: IndexMap<String, Input>) -> Response;
    pub fn get(&mut self, path: &str) -> Response;                         // path parsed with the shared path grammar
    pub fn set(&mut self, path: &str, value: Input) -> Response;
    pub fn call(&mut self, path: &str, args: Vec<Input>, kwargs: IndexMap<String, Input>) -> Response;
}
```

A future `frap-lua` crate (`mlua`, `lua54` + `vendored`, or `luau` for sandboxing) exposes `bpy.ops`/`bpy.data` userdata whose `__index`/`__newindex`/`__call` map onto these. Nothing below `frap-session` changes.

---

## 12. Session and transactions (`frap-session`)

```rust
pub struct Session {
    host: HostImpl,                            // implements frap_scene::Host: { state: State (everything rolled
                                               // back), eval: EvalCache (keyed by generation; safe across
                                               // rollback, §5.3), log: ChangeLog, reports } — disjoint from
    services: SessionServices,                 // services (F-SES-1): implements frap_op::Services; never snapshotted
    opts: SessionOptions,                      // verbosity, max_items, max_array, cwd, limits, fail_fast
    seq: u64,
}
#[non_exhaustive]                              // built from Default, then fields assigned (W0-24)
pub struct SessionOptions { pub cwd: PathBuf, pub gpu: GpuOptions, pub startup: Startup, pub verbosity: Verbosity,
                            pub max_items: usize /*100*/, pub max_array: usize /*65,536*/, pub limits: Limits, pub fail_fast: bool }
#[non_exhaustive] pub enum Startup { Factory, Empty, File(PathBuf) }
#[non_exhaustive] pub enum Verbosity { Minimal, Normal, Full }
#[non_exhaustive] pub struct Limits { pub max_elements: u64 /*50M corners*/, pub stmt_timeout: Option<Duration> }
struct SessionServices {
    render: Option<Box<dyn RenderService>>,    // created on first use by `render_factory`
    render_factory: fn(&GpuOptions) -> Result<Box<dyn RenderService>, RenderError>,  // frap_render::new_service (feature gpu) or a stub returning GPU_UNAVAILABLE;
                                               // GpuOptions lives in frap-render-scene so this compiles without `gpu` (F-REN-2)
    images: ImageCacheImpl, paths: PathResolver,
}
#[derive(Clone)] struct State { main: Main, win: WindowState, edit: EditState, prefs: Preferences, time_dirty: bool }

impl Session {
    pub fn new(opts: SessionOptions) -> Result<Session, FrapError>;    // Startup::Factory | Empty | File(path)
    pub fn exec_source(&mut self, src: &str) -> Vec<Response>;         // one Response per statement
    pub fn exec(&mut self, stmt: &Stmt) -> Response;
    fn transaction<R>(&mut self, f: impl FnOnce(&mut Self) -> Result<R, FrapError>) -> Result<R, FrapError> {
        let snapshot = self.state.clone();                              // O(#IDs), data shared
        let r = std::panic::catch_unwind(AssertUnwindSafe(|| f(self)))
            .unwrap_or_else(|p| Err(FrapError::internal_panic(p)));
        if r.is_err() || self.cancelled() { self.state = snapshot; }
        r
    }
}
```

- **One statement is one transaction**: an operator call, an assignment, a function call, or an expression with nested calls, taken whole.
- Pure reads (`get`, `print`, `poll()`) need no transaction: they never mutate `State` (edit-mode reads use the cached view, §6.3; derived values are cached in `EvalCache`, outside the snapshot). A read that calls an RNA function with side effects is a `call` and is transactional.
- Render and export work on a COW clone of `State` with animation applied at the requested frame (§7.3); the clone is dropped afterwards.
- On ERROR or CANCELLED (and on a caught panic) the snapshot is restored and the response says `"state":"unchanged"`.
- Side effects outside the scene (files from `render.render`, `export_scene.gltf`, `wm.save_as_mainfile`) are not rolled back. They run last inside the operator, after every fallible step, writing to a temporary file in the target directory and renaming on success. A failed operator never leaves a partial file.
- Limits (`frap_math::budget`, a thread-local budget the session opens around every statement): `--max-elements` (default 50M corners) is checked **before** building where an operator can estimate its output from its properties (every `mesh.primitive_*_add`: `Estimate` from the props, refused when above the limit or above Blender's `int` range, so `x_subdivisions=10**7` fails in microseconds), and again after the statement on the meshes it created or modified (a mesh in Edit Mode counts its edit BMesh, so the statement that grows it fails, not the later `mode_set`). The multiplicative edit-mode operators (`mesh.subdivide`, `subdivide_edgering`, `loopcut`, `spin`, `screw`, `bevel`) pre-estimate their output from the selection (`frap-ops-mesh/src/ops/_growth.rs`), and BMesh loop allocation is a backstop (`budget::check_corners` unwinds with `ElementsExceeded` once one mesh holds more than the limit), so runaway growth stops at the limit instead of exhausting memory (M2 stabilization round 3); all raise `RuntimeError` code `LIMIT_EXCEEDED`, with rollback. `--stmt-timeout` is a real deadline: long loops (primitive generation, polyfill, normals, looptris, corner split, BMesh ops, glTF export, render-scene building, before and after each GPU render) call the cooperative cancellation point `budget::check()`, which unwinds once the deadline has passed; the transaction catches it, restores the snapshot and answers `RuntimeError` code `TIMEOUT` (`detail.stmt_timeout_ms`). A statement that finishes after its deadline without reaching a check point fails the same way (decision log M2W0-3).
- Release builds use `panic = "unwind"`. `ed.undo`/`ed.redo` (M8) keep the last N committed `State`s.

---

## 13. JSON protocol

### 13.1 Stream contract

frap has three transports over the same `Session`:

| Mode | For | Framing |
|---|---|---|
| `frap` (raw stdin), `frap run script.py`, `frap -c "<stmt>"` | humans, shell scripts, golden cases | statements; bracket continuation; one response per statement |
| `frap serve --jsonl` | agents and harnesses driving a subprocess | one JSON request per input line, `{"id": <string or number>, "src": "<statements>", "on_error": "skip_rest" \| "continue"}`; `src` is parsed as one complete unit (§11.1), so frap never waits for more input; every response carries the request's `id`; each request ends with `{"id": …, "event": "done", "ok": <all ok>, "counts": {...}}`; the ready event is printed first and the end event at EOF, as in raw mode (same `--no-ready`/`--no-end-event` flags); a line that is not a valid request gets one `ERROR` response with code `REQUEST_INVALID` (plus a done record when its `id` could be read) and the session goes on (W3-3) |
| `frap mcp` | MCP clients (stdio, JSON-RPC 2.0: `initialize`, `tools/list`, `tools/call`) | one `Session` per connection, alive across tool calls; tools `frap_exec(statements, on_error?)` and `frap_describe(target)` (§14); the `frap_exec` result is one text block holding the JSON-Lines responses (each carrying the JSON-RPC id) then the done record, with `isError: true` when the done record is not ok; bad tool arguments give a tool result with `isError: true`, an unknown tool is JSON-RPC `-32602`; stdout carries JSON-RPC only (no ready/end events); protocol versions `2025-11-25`, `2025-06-18`, `2025-03-26`, `2024-11-05` (W3-3). Hand-written minimal server in `frap-cli/src/mcp/`, no SDK dependency (an RFC may adopt one later). |

**Batches and errors.** The stream contract (an error leaves state unchanged and the stream continues) holds at session level in every mode. Inside one `serve`/MCP request, `on_error` decides what happens to the remaining statements of **that request** after a failing one: `"skip_rest"` (default for requests) answers each of them with `status:"SKIPPED"` without running it, so an agent's batch cannot cascade into misleading errors against an unexpected state; `"continue"` runs them. Raw stdin always continues (unless `--fail-fast`).

- **stdin** (raw mode): UTF-8 statements (bracket continuation allowed, ended by a blank line or EOF, §11.1). Invalid UTF-8 on a line → `SyntaxError` for that line.
- **stdout**: JSON Lines. First line is a ready event (disable with `--no-ready`); then **exactly one object per statement**, in order; at EOF an end event (disable with `--no-end-event`).
- **stderr**: logs only (§21.5).
- **Exit code**: 0 normally (errors are reported in-band). 1 if `--fail-fast` and a statement failed (frap stops after that response). 2 on CLI usage errors or failure to start (e.g. unreadable `--file`).
- **Verbosity** (`--verbosity minimal|normal|full`, default normal): `minimal` drops `context` and `edit_mesh`; `full` adds evaluated element counts and `modifier_status` (in `context`, for the active object).

```json
{"event":"ready","frap":"0.1.0","protocol":1,"blender_api":"5.2","startup":"factory"}
{"event":"end","ok":false,"stats":{"statements":12,"errors":1,"cancelled":0}}
```

### 13.2 Response envelope

| Field | Present | Content |
|---|---|---|
| `id` | serve/MCP | the request's `id`, echoed on every response of that request |
| `seq` | always | 1-based statement counter (per session) |
| `line` | always | source line of the statement's first token (within the request in serve mode) |
| `kind` | always | `op`, `op_meta`, `get`, `set`, `call`, `print`, `noop`; `invalid` for input that never became a statement (syntax error, invalid UTF-8 on a line): exactly one `ERROR` response, no `op`/`path`/`func` |
| `ok` | always | `true` iff `status` is `FINISHED` |
| `status` | always | `FINISHED`, `CANCELLED`, `ERROR`, `SKIPPED` (serve/MCP with `on_error:"skip_rest"`) |
| `op` / `path` / `func` | by kind | idname / resolved repr path / function path |
| `reports` | always | Blender-style `[{type: INFO\|WARNING\|ERROR\|OPERATOR\|PROPERTY, message}]`, wording copied (`Removed 6 vertice(s)`) |
| `result` | op | op-specific values from `cx.result()`: created `object`/`objects`, `removed` counts, render `filepath`/`frames`, export `filepath`/`bytes`/`extensions_used`, UV `islands`, select counts |
| `value` / `rna` | get, call, print; `value` also on set | JSON value (§13.3) and its RNA type tag; a `set` response echoes the assigned value after coercion (clamped, converted) in `value`, without `rna` (§9.6) |
| `changes` | mutations | `{"added":[],"removed":[],"modified":[],"renamed":[]}` as repr paths, from the `ChangeLog` |
| `context` | normal+ (absent with `--verbosity minimal`) | `mode`, `active_object`, `selected_objects` (names, base order), `frame_current`; in edit mode also `edit_object`, `select_mode`; with `--verbosity full` also the active object's `evaluated` `{vertices, edges, faces}` and, when it has modifiers, `modifier_status` `[{name, status, message?}]` (`EVALUATED`, `SKIPPED`, `PASSED_THROUGH`, `DISABLED`, `ERROR`) |
| `edit_mesh` | edit mode, normal+ | per object in mode: `{"verts","edges","faces","tris","selected":{…},"active":{"type","index"}}` |
| `hints` | always (may be empty) | `[{code, message}]`: frap help (e.g. `NO_OP_DEFAULT`: "mesh.bevel offset=0 is a no-op; pass offset"; `ANGLE_LIKELY_DEGREES`: "rotation=0,0,90 is in radians (90 rad); did you mean radians(90)?", raised when a value of a ROTATION/ANGLE-unit prop is a whole number with magnitude > 2π; `FUNC_NOT_CALLED`: a statement that names a function without calling it, `bpy.data.images['X'].save`) |
| `error` | ERROR | §13.4 |
| `state` | CANCELLED, ERROR, SKIPPED | always `"unchanged"` |

### 13.3 Value encoding

| RNA value | JSON |
|---|---|
| f32 | shortest round-trip f32 (`0.1`, not `0.10000000149011612`) |
| NaN / ±Inf | `{"$float":"nan"}`, `{"$float":"inf"}`, `{"$float":"-inf"}` |
| Vector / Euler / Color / bpy_prop_array | plain array; type in `rna.math` (`Vector`, `Euler`, `Quaternion`, `AxisAngle`, `Matrix`, `Color`, `Array`), Euler order in `rna.order` |
| Quaternion | `[w,x,y,z]` |
| Axis-angle | `[angle,x,y,z]` |
| Matrix | row-major array of rows |
| Enum / enum flag | string / sorted array of strings |
| ID pointer | `{"$ref":"bpy.data.objects['Cube']"}` (`null` if none) |
| Non-ID struct | `{"$ptr":"bpy.data.objects['Cube'].modifiers[\"Bevel\"]","type":"BevelModifier"}` |
| Collection (named items) | `{"$coll":"bpy.data.objects","item_type":"Object","len":3,"items":["Camera","Cube","Light"]}`; `items` are names, capped by `--max-items` (default 100) with `"truncated":true` |
| Collection (nameless items: vertices, edges, polygons, loops, attribute `data`, keyframe points, …) | `{"$coll":"bpy.data.meshes['Cube'].vertices","item_type":"MeshVertex","len":8}`; no `items`. Item contents are read in bulk (below) or one by one (`vertices[3].co`). |
| Bulk array | `{"$array":[x0,y0,z0,x1,…],"shape":[N,3],"dtype":"float32","len":N,"range":[a,b],"truncated":false}`, flat row-major, capped by `--max-array` (default 65,536 scalars); when capped, `truncated:true` and `range` say which items came back |

**Bulk reads and writes** (no variables needed):
- `bpy.data.meshes['Cube'].vertices.foreach_get("co")` returns a bulk array. Blender requires a buffer argument (`foreach_get("co", seq)`); frap also accepts the one-argument form (frap extension, listed in `deviations.md`), and the Blender two-argument form with a literal list of the right length returns the filled list.
- Slices page through any collection: `….vertices[1000:2000].foreach_get("co")`, and reading `….attributes['position'].data[0:100]` returns the bulk array of that attribute's value field (`vector`, `value` or `color`) for the slice.
- `….vertices.foreach_set("co", [0,0,0, 1,0,0, …])` writes a literal flat list; a wrong length raises Blender's error (wording captured in `messages.json`). In Edit Mode the usual `RNA_EDITMODE_WRITE` rule applies.

`print(a, b)` returns `"value":[a, b]`. The exact Python repr is not reproduced; it can be added later as `py_repr` if needed.

### 13.4 Errors

```rust
// frap-rna/src/error.rs (CONTRACT)
pub enum PyExc { SyntaxError, NameError, TypeError, ValueError, KeyError, IndexError,
                 AttributeError, RuntimeError, NotImplementedError }
#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize)]
pub struct ErrorCode(pub &'static str);       // constants live in the crate that raises them
pub struct FrapError {
    pub exception: PyExc, pub code: ErrorCode,
    pub message: String,                       // Blender wording where Blender has one
    pub span: Option<Span>,                    // JSON: {"line","col","col_chars","len"}; col/len in UTF-8 bytes, col_chars in code points
    pub detail: serde_json::Map<String, serde_json::Value>,   // did_you_mean, valid, operator, argument, signature, requires, unmet
    pub hint: Option<String>,
}
```

| Code | Exception | Raised by | Example message |
|---|---|---|---|
| `SYNTAX` | SyntaxError | frap-lang | `invalid syntax` |
| `UNSUPPORTED_SYNTAX` | SyntaxError | frap-lang | `frap: variables are not supported; use a full path such as bpy.data.objects['Cube']` |
| `NAME` | NameError | frap-lang | `name 'cube' is not defined` (a lone identifier or an unknown call such as `cube_add()`) |
| `OP_UNKNOWN` | AttributeError | registry | `Calling operator "bpy.ops.mesh.foo" error, could not be found` |
| `OP_NOT_IMPLEMENTED` | NotImplementedError | registry | `mesh.knife_project exists in Blender 5.2 but is not implemented by frap` |
| `NOT_IMPLEMENTED` | NotImplementedError | executor | a statement form or path that is valid in Blender but not implemented by frap in this milestone (temporary, `deviations.md` §2) |
| `OP_ARG_UNKNOWN` / `OP_ARG_TYPE` / `OP_ARG_ENUM` / `OP_ARG_POSITIONAL` | TypeError | invoke | Blender wording (§10.2) |
| `OP_ARG_LENGTH` | ValueError | invoke | `… sequences of dimension 0 should contain 3 items, not 2` |
| `OP_POLL_FAILED` | RuntimeError | invoke | `Operator bpy.ops.mesh.bevel.poll() failed, context is incorrect` + `detail.requires`/`unmet` + `hint: "bpy.ops.object.mode_set(mode='EDIT')"` |
| `OP_EXEC_ERROR` | RuntimeError | exec | `Error: Cannot render, no camera` |
| `RNA_NO_ATTRIBUTE` | AttributeError | resolver | `'Object' object has no attribute 'foo'` |
| `RNA_READONLY` | AttributeError | resolver | `bpy_struct: attribute "type" from "Object" is read-only` |
| `CONTEXT_READONLY` | AttributeError | resolver | `bpy_struct: Context property "object" is read-only` |
| `RNA_REMOVED_IN_5X` | AttributeError | compat table | `'Action' object has no attribute 'fcurves'` + hint |
| `RNA_KEY_NOT_FOUND` | KeyError | resolver | `bpy_prop_collection[key]: key "Nope" not found` |
| `RNA_INDEX_RANGE` | IndexError | resolver | `bpy_prop_collection[index]: index 9 out of range, size 3` |
| `RNA_TYPE` / `RNA_ENUM` / `RNA_FUNC_ARGS` | TypeError | coerce, FuncArgs | Blender wording |
| `ZERO_DIVISION` | ValueError (`detail.python_exception = "ZeroDivisionError"`) | session (mathutils/arithmetic evaluation) | `division by zero` (D-25) |
| `REQUEST_INVALID` | ValueError | frap-cli (`frap serve --jsonl`) | `frap serve: invalid request: …` for a line that is not `{"id": string\|number, "src": string, "on_error"?: …}`, with a hint showing a valid request (W3-3) |
| `DESCRIBE_TARGET` | KeyError | frap-schema (`--describe`, `frap_describe`) | unknown describe target, with `detail.did_you_mean` |
| `RNA_LENGTH` | ValueError | coerce | `bpy_struct: item.attr = val: sequences of dimension 0 should contain 3 items, not 2` |
| `RNA_EDITMODE_WRITE` | RuntimeError | mesh hooks | `Mesh "Cube" is in Edit Mode; mesh data writes would be lost` |
| `RNA_NOT_IMPLEMENTED` | NotImplementedError | RNA layer (`RnaErrorKind::NotImplemented`) | part of the RNA surface frap does not implement yet (temporary) |
| `RNA_RUNTIME` | RuntimeError | RNA functions/hooks (`RnaErrorKind::Other`) | the function's own error in Blender's wording (exception as Blender raises it, usually RuntimeError) |
| `IO_ERROR` / `IO_UNSUPPORTED_FORMAT` | RuntimeError | io ops | `Error: frap cannot write .blend files; use a .frap path` |
| `GPU_UNAVAILABLE` | RuntimeError | render | no adapter / built without gpu |
| `LIMIT_EXCEEDED` | RuntimeError | session, primitive pre-checks | `Error: frap: limit exceeded (max_elements)` (pre-check: `…: the statement would create N face corners (limit M)`) |
| `TIMEOUT` | RuntimeError | session (`--stmt-timeout`) | `Error: frap: limit exceeded (stmt_timeout)`, `detail.stmt_timeout_ms`; the statement was rolled back |
| `INTERNAL` | RuntimeError | panic | includes the panic message |

`RnaError` (frap-rna, RFC 0001 F-ERR-1) is the RNA-layer error: `{ kind: RnaErrorKind, exception: PyExc, message, hint, detail }`, where `message` has **no** context prefix and `kind` selects the code family; callers convert with `into_frap(prefix, code_for_kind)` because Blender's prefix depends on the site (`bpy_struct: item.attr = val: ` vs `Converting py args to operator properties: `), or with `From<RnaError> for FrapError` (no prefix, default code of the kind). RNA codes are constants in `frap_rna::codes`.

Library crates use `thiserror` enums converting `Into<FrapError>`. Only `frap-cli` uses `anyhow`. `frap --describe=errors` lists every registered code (collected through a `linkme` slice of `ErrorCodeDoc`).

### 13.5 Example session

Input (stdin):

```
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))
data.objects['Cube.001'].location
bpy.ops.mesh.primitive_cube_add(sizee=2)
```

Output (stdout, one line per object; wrapped here):

```json
{"event":"ready","frap":"0.1.0","protocol":1,"blender_api":"5.2","startup":"factory"}
{"seq":1,"line":1,"kind":"op","op":"mesh.primitive_cube_add","ok":true,"status":"FINISHED","reports":[],
 "result":{"object":{"$ref":"bpy.data.objects['Cube.001']"}},
 "changes":{"added":["bpy.data.objects['Cube.001']","bpy.data.meshes['Cube.001']"],"removed":[],
            "modified":["bpy.data.collections['Collection']","bpy.data.scenes['Scene']"],"renamed":[]},
 "context":{"mode":"OBJECT","active_object":"Cube.001","selected_objects":["Cube.001"],"frame_current":1},"hints":[]}
{"seq":2,"line":2,"kind":"get","ok":true,"status":"FINISHED","path":"bpy.data.objects['Cube.001'].location",
 "value":[0,0,1],"rna":{"type":"FLOAT","subtype":"TRANSLATION","array":[3],"math":"Vector"},"reports":[],
 "context":{"mode":"OBJECT","active_object":"Cube.001","selected_objects":["Cube.001"],"frame_current":1},"hints":[]}
{"seq":3,"line":3,"kind":"op","op":"mesh.primitive_cube_add","ok":false,"status":"ERROR","reports":[],"state":"unchanged",
 "error":{"exception":"TypeError","code":"OP_ARG_UNKNOWN",
          "message":"Converting py args to operator properties:: keyword \"sizee\" unrecognized",
          "span":{"line":3,"col":33,"col_chars":33,"len":5},
          "detail":{"operator":"mesh.primitive_cube_add","argument":"sizee","did_you_mean":["size"],
                    "valid":["size","calc_uvs","enter_editmode","align","location","rotation","scale"]},"hint":null},
 "context":{"mode":"OBJECT","active_object":"Cube.001","selected_objects":["Cube.001"],"frame_current":1},"hints":[]}
{"event":"end","ok":false,"stats":{"statements":3,"errors":1,"cancelled":0}}
```

A CANCELLED response looks like `{"seq":4,"kind":"op","op":"object.join","ok":false,"status":"CANCELLED","state":"unchanged","reports":[{"type":"WARNING","message":"Active object is not a selected mesh"}], …}`. The response schema is published by `frap --describe=protocol`.

The same in `frap serve --jsonl` (one request; the second statement fails, the third is skipped):

```json
{"id":7,"src":"bpy.ops.mesh.primitive_cube_add()\nbpy.ops.mesh.bevel(offset=0.1)\nbpy.ops.object.shade_smooth()"}
```
```json
{"id":7,"seq":1,"line":1,"kind":"op","ok":true,"status":"FINISHED", …}
{"id":7,"seq":2,"line":2,"kind":"op","ok":false,"status":"ERROR","state":"unchanged","error":{"exception":"RuntimeError","code":"OP_POLL_FAILED","message":"Operator bpy.ops.mesh.bevel.poll() failed, context is incorrect","detail":{"requires":["MODE_EDIT_MESH"],"unmet":["MODE_EDIT_MESH"]},"hint":"bpy.ops.object.mode_set(mode='EDIT')"}, …}
{"id":7,"seq":3,"line":3,"kind":"op","ok":false,"status":"SKIPPED","state":"unchanged", …}
{"id":7,"event":"done","ok":false,"counts":{"finished":1,"error":1,"cancelled":0,"skipped":1}}
```

---

## 14. Schema export (`frap-schema`)

```
frap --describe                          # manifest (default)
frap --describe=jsonschema               # one flattened JSON Schema 2020-12 per operator input + protocol envelope
frap --describe=tools-anthropic          # LLM tool definitions (also tools-openai)
frap --describe=protocol                 # response envelope schema (also: errors)
frap --describe op:mesh.bevel            # one operator
frap --describe struct:BevelModifier     # one struct
frap --describe --filter 'mesh.*' --implemented-only
```

Also in-stream: `bpy.ops.frap.describe(target='mesh.bevel')` / `frap.describe target=Object.location`.

- **Manifest** (Blender-RNA-shaped): `operators[]` with `idname`, `bl_idname`, `label`, `description`, `bl_options`, `poll` (`requires` list from the declarative poll, human text, dumped matrix), `properties[]` (`identifier`, `type`, `subtype`, `unit` (angles shown as `"ROTATION (radians)"`), `array`, `default`, `hard_min/max`, `soft_min/max`, `enum_items`, `is_enum_flag`, `hidden`, `skip_save`, `dynamic_enum` + `context_rule`, `unset_semantics`, `frap_note`, `frap_extension`), `frap:{status, tier, headless, notes}`; `structs[]` with props, functions and python-only members; `context_members[]`; `modifier_types[]` with evaluator support; `node_types[]`; `defaults_that_surprise[]` (`export_apply=False`, `export_cameras=False`, `export_lights=False`, `light_add` 10 W, `bevel offset=0`, `vertices_smooth factor=0`, rest frame 0, …).
- Unimplemented operators appear with `status:"not_implemented"`, so an agent can tell "not in Blender" from "not yet in frap". Hidden props are always included and flagged.
- **jsonschema**: no `$ref`; enums → `enum`; enum flags → `array` + `uniqueItems`; fixed arrays → `minItems = maxItems`; defaults included.
- **tools-***: hundreds of tools overwhelm agents, so frap emits a **small fixed toolset**: `frap_exec(statements: string, on_error?: "skip_rest"|"continue")`, `frap_describe(target: string)`, and optionally per-namespace tools whose `call` argument is an `anyOf` of `{op: const, args}` objects, one per operator (provider schema subsets reject `oneOf`; W2-9). These definitions are served as-is by `frap mcp`; for other harnesses, `docs/agents.md` shows how to back them with one long-lived `frap serve --jsonl` process (the process holds the `Session` between tool calls).
- `schemars` generates only static parts (envelope, errors, `.frap` manifest). Everything else is built from the `frap-spec-*` tables plus the registry, so it cannot drift. Snapshots (`insta`) are per namespace, so namespaces never conflict.

---

## 15. The `.frap` project format (`frap-project`)

```
scene.frap  (zip; entries in this exact order; all timestamps 1980-01-01 00:00:00)
├─ mimetype                     "application/vnd.frap+zip"            (Stored, first)
├─ manifest.json                {"format":"frap","schema_version":1,"blender_api":"5.2",
│                                "generator":"frap 0.1.0","buffers":[…],"images":[…]}
├─ scene.json                   all datablocks + window/edit state, pretty-printed, Deflated
├─ buffers/<sha256-32hex>.bin   little-endian raw arrays (attributes, face offsets, deform verts, shape keys, keyframes)
└─ images/<sha256-32hex>.<ext>  original encoded bytes (PNG/JPEG/WebP/EXR/HDR/KTX2), Stored
```

```jsonc
{"window":{"scene":"Scene","view_layer":"ViewLayer"},
 "objects":{"Cube":{"type":"MESH","data":{"$id":"ME","name":"Cube"},"location":[0,0,0],"rotation_mode":"XYZ",
            "rotation_euler":[0,0,0],"rotation_quaternion":[1,0,0,0],"scale":[1,1,1],"parent":null,
            "matrix_parent_inverse":[[1,0,0,0],[0,1,0,0],[0,0,1,0],[0,0,0,1]],"mode":"OBJECT",
            "modifiers":[{"name":"Bevel","type":"BEVEL","persistent_uid":1,"width":0.1,"…":"…"}],
            "material_slot_overrides":[]}},
 "meshes":{"Cube":{"face_offsets":{"$buf":"5e1c…","dtype":"i32","len":7},
           "attributes":[{"name":"position","domain":"POINT","data_type":"FLOAT_VECTOR",
                          "data":{"$buf":"9a0f…","dtype":"f32x3","len":8}},
                         {"name":"sharp_face","domain":"FACE","data_type":"BOOLEAN","data":[true,true,true,true,true,true]}],
           "materials":[{"$id":"MA","name":"Material"}]}},
 "scenes":{"Scene":{"view_layers":[{"name":"ViewLayer","bases":[{"object":"Cube","select":true}],"active":"Cube"}],"…":"…"}}}
```

- **Keys are RNA identifiers** (data struct fields are named after them, §5.6). All values of implemented properties are written, not just non-defaults, so diffs stay stable when defaults change.
- **References** are `{"$id": code, "name": name}`, plus `"library": "<library name>"` for linked IDs; `IdKey`s are regenerated on load. ID collections are objects keyed by name, in §5.4 order; linked IDs are written under `"linked": {"<library name>": {…}}` inside their collection, so a local and a linked ID with the same name both round-trip. `libraries` is a collection like any other (filepath stored as given). Embedded IDs are written inline in their owner (`materials.X.node_tree`).
- **Buffers** are content-addressed (sha256, first 32 hex chars): equal data is stored once and `scene.json` diffs show *which* buffer changed. Arrays shorter than 64 elements are inline.
- **Serialization**: serde derives on the data types; `Arc<Vec<T>>` fields use `#[serde(with = "frap_rna::serde_buf")]`, which goes through a scoped `BufferSink`/`BufferSource` (thread-local during save/load), so data crates never depend on `frap-project`. Generated plain structs get serde derives automatically.
- **Saved state**: all datablocks, bases (selection), active object, `Object.mode`, cursor, tool settings, window state (`scene`, `view_layer`), glTF add-on preferences. Edit BMeshes are converted to `Mesh` (the cached view, §6.3) for writing; objects saved in Edit Mode re-enter it on load. Select history is not saved.
- **Orphans** (0 users, no fake user) are not written, as in Blender [OBJ §1.11]. The in-memory state is unchanged; the response reports how many were dropped.
- **Operators**: `wm.save_as_mainfile(filepath, compress, relative_remap, copy, …)`, `wm.save_mainfile`, `wm.open_mainfile(filepath, load_ui, use_scripts, …)`, `wm.read_homefile(use_empty)`, `wm.read_factory_settings`, `wm.revert_mainfile`. Blender props that do not apply are accepted and ignored with an INFO report. `bpy.data.filepath` and `is_dirty` update like Blender (`Saved "x.frap"`); `copy=True` keeps `filepath`; `//` paths resolve relative to the `.frap` (or the cwd when unsaved). A `.blend` path → `IO_UNSUPPORTED_FORMAT`.
- **Versioning**: integer `schema_version`. `frap-project/src/migrate/v{N}_to_v{N+1}.rs`, one migration per file, operates on `serde_json::Value` before typed deserialization. Newer versions are refused. Unknown fields fail the load unless `--lenient`. `blender_api` is recorded so 5.3 RNA renames can be migrated.
- **Determinism**: fixed entry order, fixed timestamps, `serde_json` `preserve_order`, shortest-f32 floats, fixed Deflate level, no platform-dependent data. Saving the same state twice is byte-identical (test).

---

## 16. Materials and shader node trees

### 16.1 Node trees (`frap-nodes`, CONTRACT)

```rust
// header.is_embedded_data is true for Material/World trees (Root::Embedded, §9.3); false for bpy.data.node_groups.
// Image Texture / Environment nodes hold `image: Option<IdRef<ImageId>>` in their PropBag (marker kind, §5.1).
pub struct NodeTree { pub header: IdHeader, pub ty: NodeTreeType /* SHADER | GEOMETRY */, pub nodes: Vec<Node>,
                      pub links: Vec<Link>, pub interface: Vec<InterfaceSocket>, pub next_node_uid: u32,
                      pub next_interface_uid: u32 /* bNodeTreeInterface.next_uid, M34W1-3 */,
                      pub animation_data: Option<AnimData> }
pub struct Node { pub uid: u32, pub name: SmolStr, pub label: String, pub bl_idname: &'static str,
                  pub location: [f32; 2], pub width: f32, pub parent: Option<u32>, pub mute: bool, pub hide: bool,
                  pub inputs: Vec<Socket>, pub outputs: Vec<Socket>, pub props: PropBag /* generated per type */ }
// Socket and link names are SmolStr: group sockets take them from user data and .frap load cannot
// produce &'static str without leaking (RFC 0001 F-NODE-1).
pub struct Socket { pub name: SmolStr, pub identifier: SmolStr, pub ty: SocketType,
                    pub enabled: bool, pub hide_value: bool, pub default_value: SocketValue }
pub struct Link { pub from_node: u32, pub from_socket: SmolStr /*identifier*/, pub to_node: u32, pub to_socket: SmolStr }
```

- `nodes.new(type="ShaderNodeTexImage")` builds the node from the generated `NodeTypeDef` (sockets, defaults, width, name = uniquified default name). Removed 5.x node types (`ShaderNodeSeparateRGB`, …) → `RuntimeError: Error: Node type ShaderNodeSeparateRGB undefined` (verified [FID S1]).
- Socket lookup by name returns the **first enabled** socket; identifiers are not keys; indices work. `availability/<node>.rs` recomputes `enabled` when a mode property changes (Mix `data_type`, Math `operation`, Separate/Combine Color `mode`, Normal Map `space`, …).
- `links.new(output, input)` replaces any existing link into a single-input socket.
- `bpy.data.materials.new()` always creates `Principled BSDF` at (−200, 100) linked to `Material Output` at (200, 100) [RNA §0.8]; `worlds.new()` creates `Background` (0.05 grey, strength 1) → `World Output`. `Material.use_nodes = False` is ignored with a DeprecationWarning report, as in 5.2.
- Built-in groups (`frap-nodes/src/builtin_groups/*.rs`): "glTF Material Output" (Occlusion, Thickness, Dispersion, Iridescence Factor, Iridescence Thickness Minimum/Maximum, …), a **local** group created on demand exactly as the add-on does, and "Smooth by Angle" (`Input_0` Mesh, `Input_1` Angle 0.5236, `Socket_1` Ignore Sharpness, `Output_2`), a **linked** group whose `library` is a `Library` entry for `geometry_nodes_essentials.blend`, created the way `shade_auto_smooth` creates it in Blender (two library entries, verified, §5.1).
- Supported node set (the glTF exporter's vocabulary; see OPERATORS.md §C): Principled BSDF, Material Output, Image Texture, Normal Map, Bump (read, not exported), Mapping, Texture Coordinate, UV Map, Color Attribute, Attribute, Separate/Combine Color, Separate/Combine XYZ, Mix, Math, Vector Math, RGB, Value, Invert, Emission, Transparent BSDF, Mix Shader, Add Shader, Light Path (unlit pattern), Volume Absorption, Background, Environment Texture, World Output, group nodes. Any other node type exists only if Blender has it, and is treated as "unrecognised" by lowering (WARNING + fallback).

### 16.2 Lowering (`frap-material`, CONTRACT)

```rust
pub struct LoweredMaterial {
    pub base_color: Channel<[f32; 4]>, pub metallic: Channel<f32>, pub roughness: Channel<f32>,
    pub normal: Option<NormalMap>, pub occlusion: Option<TexRef>, pub occlusion_strength: f32 /* M3 W0 */,
    pub emissive: Channel<[f32; 3]>, pub emissive_strength: f32,
    pub alpha_mode: AlphaMode,                      // Opaque | Mask { cutoff } | Blend  (from node patterns)
    pub double_sided: bool,                         // !use_backface_culling
    pub unlit: bool,
    pub ext: MaterialExts,                          // Option<Clearcoat>, Option<Sheen>, Option<Transmission>,
                                                    // Option<Volume>, ior: Option<f32>, Option<Specular>,
                                                    // Option<Iridescence>, Option<Anisotropy>, dispersion:
                                                    // Option<f32>, Option<DiffuseTransmission> (M3 W0)
    pub vertex_color: Option<VertexColorUse>,       // which colour attribute, how combined
    pub diagnostics: Vec<Report>,                   // unrecognised patterns => fallback + WARNING
}
pub struct Channel<T> { pub factor: T, pub texture: Option<TexRef> }
pub struct TexRef { pub image: IdRef<ImageId>, pub uv: UvSource /* Named | ActiveRender | Index */, pub channel: ChannelSel,
                    pub transform: Option<TexTransform>, pub sampler: Sampler, pub colorspace_is_data: bool }
pub fn lower(tree: &NodeTree, mat: &Material, images: &dyn ImageLookup) -> LoweredMaterial;
pub fn raise(lm: &LoweredMaterial, into: &mut Material, names: &mut NameAlloc);   // importer: Blender-style graph

// Extension point (M1 W0, W0-28): one entry per file in patterns/; lower/raise run every entry in `order`.
pub struct MaterialPattern { pub name: &'static str, pub order: u16,
    pub lower: fn(&LowerCx, &mut LoweredMaterial),                   // fallback + WARNING when unrecognised
    pub raise: Option<fn(&LoweredMaterial, &mut RaiseCx)> }
#[non_exhaustive] pub struct LowerCx<'a> { pub tree: &'a NodeTree, pub mat: &'a Material, pub images: &'a dyn ImageLookup }
#[non_exhaustive] pub struct RaiseCx<'a> { pub into: &'a mut Material, pub names: &'a mut NameAlloc }   // WP 1.11 adds shared fields
#[linkme::distributed_slice] pub static MATERIAL_PATTERNS: [MaterialPattern];
```

- `lower` is one **pattern recogniser per file**, each registering a `MaterialPattern` in `MATERIAL_PATTERNS` (so M3 adds files and never edits `lower`/`raise`, §22.1) (`patterns/base_color.rs`, `metallic_roughness.rs`, `normal_map.rs`, `alpha.rs`, `emission.rs`, `occlusion.rs`, `texture_transform.rs`, `vertex_color.rs`, `clearcoat.rs`, …), mirroring `io_scene_gltf2/blender/exp/material/*` and `search_node_tree.py`. Alpha mode comes from the graph, not `blend_method` [RNA §0.9].
- **Both the exporter and the renderer consume `LoweredMaterial`.** Whatever exports correctly renders correctly.
- Round-trip law (tested for every fixture material): `lower(raise(lower(x))) == lower(x)`.

---

## 17. glTF/GLB import and export

### 17.1 `frap-gltf`: format only

```
crates/frap-gltf/src/
  json/{root,asset,scene,node,mesh,accessor,buffer,material,texture,image,sampler,camera,skin,animation}.rs
  ext/<extension_name>.rs        # one file per extension: typed struct + validation
  glb.rs                         # container (JSON chunk padded 0x20, BIN chunk padded 0x00, unknown chunks skipped)
  accessor.rs                    # encode/decode: normalized ints, sparse, byteStride, min/max, quantized types
  builder.rs                     # BufferBuilder: aligned views; interleave option
  validate.rs                    # indices in range, required extensions present
```

```rust
pub struct Document { pub asset: Asset, pub scene: Option<u32>, pub scenes: Vec<Scene>, pub nodes: Vec<Node>,
    pub meshes: Vec<Mesh>, pub materials: Vec<Material>, pub textures: Vec<Texture>, pub images: Vec<Image>,
    pub samplers: Vec<Sampler>, pub accessors: Vec<Accessor>, pub buffer_views: Vec<BufferView>,
    pub buffers: Vec<Buffer>, pub skins: Vec<Skin>, pub cameras: Vec<Camera>, pub animations: Vec<Animation>,
    pub extensions_used: Vec<String>, pub extensions_required: Vec<String>,
    pub extensions: Extensions, pub extras: Option<Box<RawValue>> }
pub struct Extensions(IndexMap<String, serde_json::Value>);          // lossless for unknown extensions
impl Extensions { pub fn get<E: Extension>(&self) -> Result<Option<E>, GltfError>; pub fn set<E: Extension>(&mut self, e: Option<E>); }
pub trait Extension: Serialize + DeserializeOwned { const NAME: &'static str; const REQUIRED_BY_DEFAULT: bool = false; }
pub fn read_glb(bytes: &[u8]) -> Result<Glb, GltfError>;   pub fn write_glb(g: &Glb, opts: &WriteOpts) -> Vec<u8>;
```

Typed extensions: KHR_lights_punctual; KHR_materials_{emissive_strength, transmission, clearcoat, sheen, ior, specular, volume, iridescence, anisotropy, dispersion, unlit, variants, diffuse_transmission, pbrSpecularGlossiness (read)}; KHR_texture_transform; KHR_mesh_quantization; KHR_draco_mesh_compression; EXT_meshopt_compression; KHR_meshopt_compression; EXT_texture_webp; KHR_texture_basisu; EXT_mesh_gpu_instancing; KHR_animation_pointer; KHR_node_visibility. Unknown extensions and `extras` round-trip untouched.

### 17.2 `frap-io-gltf`: io_scene_gltf2 5.2.41 semantics

The module structure mirrors the add-on so porting and review split across agents:

```
crates/frap-io-gltf/src/export/
  settings.rs          # generated ExportSettings (Blender's 110 props) + FrapExportExt (frap_* props)
  scenes.rs            # one glTF scene per Blender scene, or the active one (use_active_scene)        (M3)
  vtree.rs             # roots sorted natural + case-insensitive; filters (selection, visible, renderable, collection, …)
  rest_frame.rs        # rest pose at frame 0 unless export_current_frame, evaluated on a State clone (§7.3)
  nodes.rs             # TRS per object (Y-up swizzle, identity snapping), post-order node array
  mesh/primitives.rs   # driver: runs the three extension-point slices below, in slice order sorted by `order`
  mesh/extract/        # PRIMITIVE_EXTRACTORS: triangles.rs (M1: one primitive per used slot, looptris, corner split,
                       #   byte-sorted unique), loose_edges.rs -> LINES and loose_points.rs -> POINTS
                       #   (use_mesh_edges / use_mesh_vertices), pointcloud.rs -> POINTS              (M3)
  mesh/writers/        # PRIMITIVE_ATTRIBUTE_WRITERS: normal.rs, texcoord.rs (M1); color.rs incl. fake white COLOR_0,
                       #   tangent.rs, extra_uvs (M3); skin.rs JOINTS/WEIGHTS (M6)
  mesh/post/           # PRIMITIVE_POST_PROCESS: morph_targets.rs (M6), quantize.rs, meshopt.rs, draco.rs (M7)
  instancing.rs        # export_gpu_instances: EXT_mesh_gpu_instancing for children of an Empty (add-on rule) (M7)
  materials/*.rs       # one file per glTF material feature, consuming LoweredMaterial; materials/ext/<name>.rs:
                       #   one KHR_materials_* exporter each (MATERIAL_EXT_EXPORTERS, M3 W0)
  images.rs            # export_image_format AUTO/JPEG/WEBP/NONE, ORM packing, passthrough of original bytes
  cameras.rs lights.rs # W -> cd / lux conversion, spot cones, AREA warning
  skins.rs animation/*.rs                                                                        (M6)
  compress.rs          # Draco / meshopt / quantization through codec traits                     (M7)
  write.rs             # key order, empty pruning, extensionsRequired rules, GLB / .gltf / embedded
crates/frap-io-gltf/src/import/
  materials_ext/<name>.rs (MATERIAL_EXT_IMPORTERS, M3 W0)
  settings.rs scenes.rs nodes.rs mesh.rs (merge_vertices, shading detection; TRIANGLES, TRIANGLE_STRIP and
  TRIANGLE_FAN converted to faces, LINES/LINE_STRIP/LINE_LOOP to loose edges, POINTS to loose vertices or,
  with import_point_as_pointcloud, a POINTCLOUD object) materials.rs (raise) images.rs
  cameras.rs lights.rs skins.rs (bone_heuristic, guess_original_bind_pose) animation.rs decompress.rs variants.rs
```

```rust
// frap-io-gltf/src/export/mesh/ext.rs (CONTRACT, M1 W0). Later milestones add files, never edit primitives.rs.
// Driver: (1) writers' `key` fill one SplitKey (the add-on's per-corner `dots`), (2) extractors dedupe on
// (vertex index, key) and record a representative corner + vertex per glTF vertex, (3) writers' `run`, (4) post.
#[non_exhaustive] pub struct MeshExportCx<'a> { pub main: &'a Main, pub mesh: &'a Mesh,
    pub object: Option<IdRef<ObjectId>>,                     // vertex groups/armature (M6), object-linked slots; None if shared
    pub settings: &'a ExportSettings }                       // built by the driver with MeshExportCx::new; fields added additively
pub struct SplitKey { pub corners: usize, pub columns: Vec<KeyColumn> }   // KeyColumn { writer, stride, words: Vec<u32> }
#[non_exhaustive] pub struct PrimitiveDraft { pub mode: u32, pub material_slot: usize, pub positions, pub normals, pub texcoords,
    pub indices: Vec<u32>, pub corners: Vec<u32>, pub vertices: Vec<u32>,
    pub attributes: IndexMap<SmolStr, AttrColumn>,           // COLOR_n, TANGENT, JOINTS_n, WEIGHTS_n, … in output order
    pub targets: Vec<IndexMap<SmolStr, AttrColumn>> }        // morph targets (M6)
pub struct AttrColumn { pub values: AttrValues /* F32x2|F32x3|F32x4|U8x4|U16x4, non_exhaustive */, pub normalized: bool }
pub struct PrimitiveExtractor   { pub name: &'static str, pub order: u16, pub run: fn(&MeshExportCx, &SplitKey, &mut Vec<PrimitiveDraft>) }
pub struct PrimitiveAttributeWriter { pub name: &'static str, pub order: u16,
    pub key: Option<fn(&MeshExportCx, &mut SplitKey) -> Result<(), ExportError>>,   // None: per-vertex data (skin)
    pub run: fn(&MeshExportCx, &SplitKey, &mut PrimitiveDraft) -> Result<(), ExportError> }
pub struct PrimitivePostProcess { pub name: &'static str, pub order: u16, pub run: fn(&MeshExportCx, &mut PrimitiveDraft, &mut BufferBuilder) -> Result<(), ExportError> }
#[linkme::distributed_slice] pub static PRIMITIVE_EXTRACTORS: [PrimitiveExtractor];
#[linkme::distributed_slice] pub static PRIMITIVE_ATTRIBUTE_WRITERS: [PrimitiveAttributeWriter];
#[linkme::distributed_slice] pub static PRIMITIVE_POST_PROCESS: [PrimitivePostProcess];
```

M3+M4 W0 extension points (decision log M34W0-8): `material_ext.rs` holds `MATERIAL_EXT_EXPORTERS` (`fn(&mut ExtExportCx, &LoweredMaterial, &mut gltf::Material)`, run after the core members) and `MATERIAL_EXT_IMPORTERS` (`fn(&mut dyn ExtImportHost, &gltf::Material, &mut LoweredMaterial)`, run before `raise`); `ExportSettings::props` carries every typed operator prop and a feature file declares the props it honours in `SUPPORTED_EXPORT_PROPS` (the operator's "not supported yet" WARNING asks `is_supported`); `Gather::state::<T>()` gives each feature file its own export-wide state; `scenes.rs` gathers the scene(s), `nodes.rs` asks `cameras::gather`, `lights::apply` and `extras::object`, `vtree.rs` asks `cameras::exported`/`lights::exported`; `ExportSettings::evaluated_meshes` carries the `export_apply=True` meshes the operator evaluated (read by `apply.rs`).

- **Defaults are Blender's**: `export_apply=False`, `export_cameras=False`, `export_lights=False`, `export_yup=True`, `export_force_sampling=True`, `export_vertex_color='MATERIAL'`, … (generated, never typed). With every `frap_*` option at default the output follows Blender's rules [GLTF §4, §7.1]: non-interleaved views, gather order, omission rules, rest at frame 0, fake white COLOR_0, BYTE_COLOR → USHORT, UNSIGNED_SHORT/INT indices, `extensionsRequired` for lights and texture_transform, the `fps_base` time formula (WARNING when `fps_base != 1`). Byte-identity with Blender is a non-goal; goldens compare canonically.
- **frap-only export options** (listed in `--describe` with `frap_extension: true`), only for features Blender's exporter has no prop for (audited by the codegen test of §4.2): `frap_compat` (`'BLENDER'` default | `'SPEC'`, drops quirks), `frap_texture_format` (`'AUTO'` | `'KTX2'`), `frap_basisu_mode` (`'ETC1S'` | `'UASTC'`), `frap_mesh_quantization`, `frap_interleave`, `frap_node_visibility`, `frap_diffuse_transmission`, `frap_draco_backend`. **GPU instancing uses Blender's own `export_gpu_instances`** (verified present) with the add-on's rule (instances are children of an Empty sharing a mesh), not a frap option.
- **Scenes.** All Blender scenes export as glTF `scenes[]` with the active scene as `scene`, or only the active one with `use_active_scene=True` (verified prop); import creates one Blender scene per glTF scene and makes the glTF default scene active.
- **Non-triangle primitives.** Export writes loose edges as LINES (`use_mesh_edges`) and loose vertices as POINTS (`use_mesh_vertices`), as the add-on's `primitive_extract.py` does (verified); import converts strips, fans, line strips/loops and points (above). `import_point_as_pointcloud=True` (verified prop) creates a `PointCloud` object (§5.6).
- **Units.** `scene.unit_settings.scale_length` is **ignored on export and divided out on import**: the add-on's only use is in `blender_gltf.py:68` (verified). frap reproduces the asymmetry; `docs/research/gltf-and-blender-io.md` lines 300 and 590 disagree on this and are superseded by this paragraph.
- **File paths.** The add-on passes `self.filepath` to the writer raw (`__init__.py` line 1152, verified), so in Blender `filepath='//out/part.glb'` writes to the absolute path `/out/part.glb`. frap resolves `//` like every other Blender path (relative to `bpy.data.filepath`, or the cwd when unsaved). This is a listed deviation in `docs/deviations.md`; goldens never use `//` export paths.
- **KHR_materials_variants** needs the add-on preference `KHR_materials_variants_ui` (§4.1); `Scene.gltf2_KHR_materials_variants_variants` and the per-mesh variant props exist only while it is on, in frap as in Blender, and importing a file with variants turns it on.
- `export_use_gltfpack=True` maps the `export_gltfpack_*` props onto in-process meshopt with gltfpack-equivalent semantics (INFO report); no external binary.
- The exporter reads typed scene data directly and walks the same graph as the add-on. Materials come from `frap-material::lower`.
- Import is the inverse port: merge rules, `import_shading` from normals, `bone_heuristic` (BLENDER = +90° X roll), `guess_original_bind_pose`, NLA stashing of animations, material graph rebuild via `raise` (incl. the glTF Material Output group and Volume Absorption), lights with `range` ignored.
- **Round-trip law (differential).** GLB → frap → GLB is **not** expected to equal the source: Blender itself adds a fake COLOR_0, drops tangents (`export_tangents` off by default), merges vertices, normalises samplers and texture transforms, re-encodes sparse and strided accessors and converts strips and fans. The law is: for every file in the allow-listed fixture set (`tests/data/gltf-roundtrip/allow.toml`), `gltf_canonical(frap import → frap export) == gltf_canonical(Blender import → Blender export)` with default options.
- Codec traits (`frap-codec-mesh`, `frap-imbuf`): `DracoCodec { encode(prim, quantization, level), decode(bytes, attr_map) }`, `MeshoptCodec { encode_vertex, encode_index, decode }`, `ImageCodec { decode, encode(EncodeFormat) }`.

---

## 18. Animation, armatures, shape keys

- **`frap-anim`** (pure data + evaluation): `Action { slots, layers: [{ strips: [{ channelbags: [{ slot_handle, groups, fcurves }] }] }] }`; `FCurve { data_path, array_index, group, keyframe_points, extrapolation, mute, modifiers (P2) }`; `Keyframe { co, handle_left/right, handle_*_type, interpolation, easing, back, amplitude, period, type }`; `AnimData { action, action_slot_handle, last_slot_identifier, nla_tracks, use_nla, action_blend_type, action_influence, action_extrapolation }`; `evaluate_fcurve(&FCurve, frame) -> f32` (Bézier via a port of `BKE_nurb_handle_calc` semantics, 13 interpolations with easings, extrapolation); `evaluate_anim(&AnimData, actions, frame) -> Vec<(data_path, index, value)>` incl. NLA blending.
- **Keying** (`frap-ops-anim/src/keying.rs`): `keyframe_insert(data_path, *, index=-1, frame=None, group="", options=set(), keytype='KEYFRAME') -> bool` creates `<IDName>Action`, slot `OB<name>`/`KE<name>`/`NT<name>`, one layer, one KEYFRAME strip, group "Object Transforms" (or the bone name), BEZIER + AUTO_CLAMPED keys from preferences. Shape-key keys go into the **object's** action under a `KE` slot; material node-tree keys get their own action with slot `NTShader Nodetree`, stored in the **embedded** node tree's `AnimData` and addressed through `Root::Embedded` (§9.3) [RNA §0.3–0.4]. `action.fcurves` → `RNA_REMOVED_IN_5X` with the 5.x path as hint.
- **Applying animation** (`frap-eval/src/animate.rs`) writes channels into originals through `Host::set_path` (ID-relative `path_from_id` paths, owners `Root::Id` or `Root::Embedded`). It runs only at evaluation boundaries (§7.3): `scene.frame_set`, operator pipeline step 4 and `view_layer.update()` when time is dirty, `evaluated_depsgraph_get`. `render.render(animation=True)` and the glTF exporter's sampler apply it on a per-frame COW clone of `State`, never on the session's originals.
- **Armatures**: `Armature.bones` hold rest data; `EditArmature { edit_bones }` lives in `EditState`; `Object.pose.bones` hold channels (`rotation_mode` default `QUATERNION`); `frap-eval/src/pose.rs` computes pose matrices; the ARMATURE modifier does linear blend skinning (dual quaternion P2) by vertex-group name ↔ bone name.
- **Shape keys**: the `Key` ID holds absolute positions per key block; evaluation mixes them relatively; in Edit Mode each key is a BMesh `ShapeLayer` and basis offsets propagate on write-back.

---

## 19. Renderer

### 19.1 Data flow

```
State clone + animation at frame ──► frap-render-scene::build(&dyn HostRead, &mut dyn Evaluator, &RenderRequest) -> Result<RenderScene, RenderError>   (CPU, no wgpu)
                       │  plain data: corner-split meshes, LoweredMaterials, decoded images, lights, camera, world, settings
                       ▼
OpCtx::services().render() ──► dyn RenderService = frap-render::Renderer { gpu (lazy), engines: {Workbench, Eevee}, cache } ──► Film (scene-linear f32 RGBA)
                       ▼
frap-color::display_transform(&film.rgba, film.width, film.height, &ViewSettings, &DisplaySettings) -> DisplayImage ──► frap-imbuf encode ──► file + "Render Result"
```

### 19.2 Contracts

```rust
// frap-render-scene/src/types.rs (CONTRACT)
pub struct RenderScene {
    pub resolution: [u32; 2],                       // resolution_x/y × percentage
    pub camera: RCamera,                            // view, projection (sensor_fit, shift, clip, ortho_scale), pixel aspect
    pub objects: Vec<RObject>,                      // sorted by object name (determinism)
    pub meshes: IndexMap<MeshKey, Arc<RMesh>>,      // MeshKey = evaluated fingerprint -> GPU cache key
    pub materials: IndexMap<MatKey, Arc<LoweredMaterial>>,
    pub images: IndexMap<ImageKey, Arc<DecodedImage>>,
    pub lights: Vec<RLight>,                        // Blender units preserved (W, W/m², spot_size, spot_blend, radius)
    pub world: RWorld,                              // Color { rgb, strength } | Hdri { image, rotation, strength }
    pub settings: RSettings,                        // engine, taa_render_samples, shadows, film_transparent,
                                                    // display.shading (Workbench), view_settings, frame
}
// RSettings (additive, W1-15/W2-7): Workbench viewport values, read in MATERIAL colour mode / for the background
//   pub workbench_materials: IndexMap<MatKey, RWorkbenchMaterial>,   // Material.diffuse_color/metallic/roughness
//   pub workbench_background: Option<[f32; 4]>,                       // World.color (VIEWPORT: shading.background_color)
pub struct RWorkbenchMaterial { pub diffuse_color: [f32; 4], pub metallic: f32, pub roughness: f32 }
pub struct RMesh { pub positions: Vec<[f32; 3]>, pub normals: Vec<[f32; 3]>, pub uvs: Vec<Vec<[f32; 2]>>,
                   pub colors: Vec<Vec<[f32; 4]>>, pub tangents: Option<Vec<[f32; 4]>>, pub indices: Vec<u32>,
                   pub submeshes: Vec<(u32 /*material slot*/, Range<u32>)>, pub edges: Vec<[u32; 2]> }

// frap-render-scene/src/types.rs (CONTRACT): the only GPU-facing trait the rest of frap sees
pub trait RenderService: Send {
    fn render(&mut self, scene: &RenderScene) -> Result<Film, RenderError>;
    fn render_tiles(&mut self, scenes: &[RenderScene]) -> Result<Vec<Film>, RenderError>;  // contact sheet
    fn adapter_info(&self) -> AdapterSummary;
}
pub struct Film { pub width: u32, pub height: u32, pub rgba: Vec<[f32; 4]> }   // scene-linear, straight alpha
// Defined here (F-REN-1/F-REN-2) so the session's render factory compiles without the gpu feature:
pub struct GpuOptions { pub backend: Option<String>, pub adapter: Option<String>, pub force_fallback: bool }  // from_env()
pub struct RenderRequest { pub frame: Option<i32>, pub resolution: Option<[u32; 2]>,
                           pub camera: Option<RCamera>, pub objects: Option<Vec<SmolStr>> }

// frap-render (CONTRACT; lib.rs re-exports gpu.rs, engine.rs, service.rs; GpuOptions re-exported from frap-render-scene)
pub fn new_service(opts: &GpuOptions) -> Result<Box<dyn RenderService>, RenderError>;  // used by frap-session
pub trait Engine { fn render(&mut self, gpu: &Gpu, cache: &mut GpuCache, scene: &RenderScene) -> Result<Film, RenderError>; }
pub struct Gpu { pub device: wgpu::Device, pub queue: wgpu::Queue, pub info: wgpu::AdapterInfo }
impl Gpu { pub fn new(opts: &GpuOptions) -> Result<Gpu, RenderError>; }   // FRAP_WGPU_BACKEND, FRAP_ADAPTER, force_fallback
```

Offscreen targets are textures with `RENDER_ATTACHMENT | COPY_SRC`; TAA and soft-shadow accumulation use **`Rgba32Float`** (an `Rgba16Float` accumulator has an 11-bit mantissa and bands after a few dozen samples); read back through buffers with rows padded to 256 bytes. The device is created on the first render and reused. `GpuCache` keeps meshes by `MeshKey` and textures by `ImageKey`.

**Shaders and start-up.** Each engine has **one uber-shader** (Workbench, EEVEE forward, shadow, world/IBL) whose material features are uniform flags, not pipeline permutations: every CLI run starts cold and permutation compiles are slow on lavapipe and WARP. Pipelines are created once per engine and target format.

**Backends.** macOS: Metal. Linux: Vulkan. Windows: Vulkan first; DX12 as fallback and in CI (WARP), with `Dx12Compiler::Fxc` selected explicitly. The `static-dxc` feature is not used (it downloads binaries at build time, rule 2.2.9); FXC's slowness on loop-heavy shaders is bounded by the uber-shader design and measured in CI. `FRAP_WGPU_BACKEND` overrides the order.

### 19.3 Engines

| Engine | Implementation |
|---|---|
| `BLENDER_WORKBENCH` (M1 basic, M2 complete) | Reads `scene.display.shading` (factory: `type='SOLID'`, `light='STUDIO'`, `color_type='MATERIAL'`, verified) and `scene.display.light_direction`/`shadow_shift`. `light`: STUDIO (studio rigs baked from Blender, provenance recorded), MATCAP, FLAT. `color_type`: MATERIAL (`diffuse_color`), OBJECT, RANDOM (Blender's name hash), VERTEX, TEXTURE, SINGLE. `show_xray`/`xray_alpha`, wireframe (`type='WIREFRAME'`, `show_wireframes`), `show_object_outline`, `show_shadows`, `show_cavity` (P2), `background_type`. |
| `BLENDER_EEVEE` (M5) | Forward PBR (GGX + Lambert, multiscatter approximation) on `LoweredMaterial`. Light units converted with EEVEE's power functions (verified by a diffuse-plane golden). Shadows: cube maps (point), 2D (spot), 4 cascades (sun), `Depth32Float` + PCF filtered by `shadow_soft_size`. World: constant colour or HDRI with split-sum IBL (irradiance SH, prefiltered GGX mips, BRDF LUT asset). Transparency: BLENDED sorted per object, DITHERED hashed. Transmission: screen-space refraction of the opaque pass. Emission. TAA over `eevee.taa_render_samples` with a fixed Halton sequence (also soft shadows and AA). `film_transparent`. |
| `CYCLES` | Accepted as RNA value; rendered with `BLENDER_EEVEE` + WARNING `Engine 'CYCLES' is not available in frap; rendered with BLENDER_EEVEE`. |

### 19.4 `render.render`

1. `ensure evaluated`. No `scene.camera` → `RuntimeError: Error: Cannot render, no camera`.
2. Clone `State`, apply animation at `frame_current` on the clone (Blender's render depsgraph does), build the `RenderScene` from it, render through `cx.services().render()?` (`GPU_UNAVAILABLE` when there is no adapter or the build has no `gpu` feature), colour-manage (§19.5).
3. Store as `bpy.data.images['Render Result']`. With `write_still`, write to `scene.render.filepath` following Blender's rules [OBJ §4.17]: `//` relative to the `.frap` or cwd; relative paths without `//` relative to the cwd; extension added when `use_file_extension`; `#` runs zero-padded. Formats from `image_settings.file_format/color_mode/color_depth/compression/quality`: PNG 8/16, JPEG, WEBP, OPEN_EXR, HDR, TARGA, BMP.
4. `animation=True`: for each frame in `frame_start..=frame_end` step `frame_step` (op `frame_start`/`frame_end` override only when set), apply animation at that frame on a fresh clone → render → write `frame_path(frame)` (`#` padding, else 4-digit suffix). Result lists the files. The session's originals are unchanged afterwards; `frame_current` afterwards follows Blender (pinned by a golden in WP 6.6).

### 19.5 Colour management (`frap-color`)

View transforms (AgX default, Filmic, Filmic Log, Khronos PBR Neutral, ACES 1.3, ACES 2.0, False Color) are 3D LUTs over a log2 shaper, baked from Blender by `tools/blender/bake_view_luts.py` (render an HDR lattice through each `view_transform` × `look` with `image.save_render`, fit, store in `assets/luts/*.cube`). The bake script also writes a **held-out** set of colours (random HDR values off the lattice, fixed seed) with Blender's outputs; LUT accuracy is tested only on those, never on the lattice points the LUT was fitted to. Standard and Raw are analytic. Exposure before the transform, gamma after, then display encode and dither (`render.dither_intensity`). Pure CPU and deterministic, so colour goldens need no GPU.

### 19.6 Contact sheet (`frap.render_contact_sheet`, frap extension)

```
bpy.ops.frap.render_contact_sheet(filepath='//sheet.png', views={'FRONT','RIGHT','TOP','ISO'},
    engine='BLENDER_WORKBENCH', objects='VISIBLE', view_size=512, columns=0, margin=0.05,
    show_labels=True, background=(0.2, 0.2, 0.2, 1.0), use_scene_camera=False, frame=None)
```

Views use Blender's numpad orientations (FRONT looks along +Y, RIGHT along −X, TOP along −Z, BACK/LEFT/BOTTOM opposite; ISO = factory viewport rotation; CAMERA = `scene.camera`). Orthographic synthetic cameras frame the evaluated bounds of the chosen objects; they exist only in `RenderScene`, never in `Main`. Tiles are rendered with the chosen engine and composited on the CPU with labels from an embedded bitmap font. `result: {"filepath", "tiles": [{"view", "rect"}]}`. `frame=None` (the default) renders the current frame; an explicit `None` for an INT or FLOAT property of any `frap.*` operator counts as unset (frap extension; Blender operators reject `None`).

---

## 20. Testing

### 20.1 Layers

| Layer | Tool | Location | Gate |
|---|---|---|---|
| Unit | `cargo test` | colocated `#[cfg(test)]` (pure units only; operator behaviour tests go to `frap-golden` or `crates/frap-ops-<x>-tests`, rule 2.2.11) | every PR |
| Property | `proptest`: BMesh validity after random kernel/op sequences, `coerce`, naming, parser round-trips, `.frap` round-trip | `crates/*/tests/prop_*.rs` | every PR |
| Snapshot | `insta`: IR corpus, `--describe` per namespace, responses, `.frap` scene.json | per crate, file-named | every PR |
| CLI transcripts | `trycmd`: stdin → expected JSONL (paths redacted) | `tests/cli/<milestone>/<name>.trycmd` | every PR, all 3 OS |
| Golden vs Blender | `frap-golden` (§20.2–20.4) | `tests/golden/` | fixtures every PR; live nightly |
| Render | `image-compare` SSIM + ΔE; strict self-goldens + loose Blender goldens | `tests/golden/cases/render*/` | every PR on lavapipe/WARP/Metal |
| Fuzz | `cargo fuzz`: lexer/parsers, GLB reader, `.frap` reader | `fuzz/` | nightly (M8) |
| Differential fuzz | random sequences of implemented ops run in both tools, mismatches minimised into cases | `frap-golden/src/fuzz.rs` | nightly (M8) |

### 20.2 Golden case format

```python
# tests/golden/cases/edit/extrude_region_move_top.py
# golden: startup=factory compare=objects,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_cube_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
#@frap    bpy.ops.frap.select_elements(type='FACE', indices=[5])
#@blender import bmesh; bm=bmesh.from_edit_mesh(bpy.context.object.data); bm.faces.ensure_lookup_table(); bm.faces[5].select_set(True); bmesh.update_edit_mesh(bpy.context.object.data)
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, 1)})
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(sizee=2)        # expected TypeError on both sides (T0)
```

Input files: a directory next to the case named after it with the extension `inputs` (`spec_gloss.py` → `spec_gloss.inputs/`) has its files copied into both runners' working directories first (hand-written inputs Blender cannot produce, e.g. a `KHR_materials_pbrSpecularGlossiness` glTF). Header directives: `startup` (`factory`|`empty`), `compare` (`objects`, `mesh:<name>`, `evaluated:<name>`, `materials`, `anim:<name>`, `glb:<path>`, `image:<path>`, `context`), `tier` (T0–T4), `tol`, `skip_blender` (frap-only features). Lines prefixed `#@frap` run only in frap; `#@blender` only in Blender (headless-impossible ops, frap selection helpers, loopcut emulation [MESH App. B.7]). Everything else must be valid in both tools, so cases are written in the bpy-line subset. A transpile test converts every case to short form and checks the IR is identical (§11.5).

### 20.3 Runners and the shared dump

- **Blender side**: `$BLENDER_BIN -b --factory-startup --python tools/blender/golden_runner.py -- case.py out.json`. Each statement runs with `exec` inside `bpy.context.temp_override(window, area=<VIEW_3D>, region=<WINDOW>)` of the Layout screen [OBJ §1.7]. The runner records `ok`, exception class and message per line. **Normalisation for frap's deliberate deviations**: before every statement that reads a **derived** value (`matrix_world`, `dimensions`, `bound_box`, evaluated data), it calls `bpy.context.view_layer.update()` unless time is dirty (a pending `frame_current` change must not be applied by the normaliser, since frap matches Blender there, §7.3), and, for objects in Edit Mode, `update_from_editmode()` before mesh reads (§1 #7–8). At the end it calls `dump_state.py`. The list of normalisations is `tools/blender/golden_runner.py::NORMALISE` and mirrors `docs/deviations.md`.
- **frap side**: the same statements through an in-process `Session`, then the dump is taken **through frap's own RNA** using the same property list (`tests/golden/props.toml`) that `dump_state.py` reads. The golden suite therefore also tests the RNA layer.

```rust
// frap-golden/src/state.rs (CONTRACT): identical JSON from both sides
pub struct GoldenState {
    pub blender: Option<BlenderStamp>,           // version + build hash (fixtures only)
    pub lines: Vec<LineOutcome>,                 // ok | exception class + message
    pub objects: BTreeMap<String, GObject>,      // type, parent, matrix_world, loc/rot/scale, modifiers, materials, mode
    pub meshes: BTreeMap<String, GMesh>,         // positions, edges, faces (corner verts), corner normals, uv layers,
                                                 // attributes, selection, active element, select history
    pub evaluated: BTreeMap<String, GMesh>,      // evaluated_get + to_mesh
    pub materials: BTreeMap<String, GMaterial>,  // nodes/links/socket defaults
    pub actions: BTreeMap<String, GAction>,      // slots, fcurves, keys, + sampled values per frame
    pub context: GContext,                       // active, selected (base order), mode, select_mode, cursor
    pub files: BTreeMap<String, GFile>,          // glb -> canonical glTF; png -> pixels + size
}
pub fn locate_blender() -> Option<PathBuf>;     // $BLENDER_BIN, then PATH, then ~/Applications/Blender-5.2.app/Contents/MacOS/Blender
```

### 20.4 Comparators (`frap-golden/src/compare/*.rs`, one file each)

| Comparator | Tier | Method |
|---|---|---|
| `lines` | T0 | same `ok`; same exception class; message equal after normalising quotes/paths |
| `rna_values` | T0 | listed props equal; floats within `tol` |
| `mesh_exact` | T1 | index by index: topology exact, positions/UVs within `tol`, selection and active element exact |
| `mesh_canonical` | T2 | vertices matched by quantised position (1e-4 grid, neighbourhood tie-break); faces as rotation-normalised position cycles (multiset); UVs and corner attributes keyed by (face key, corner position) |
| `invariants` | T3 | case-declared: UV bounds, no overlap, area ratio, manifold, Euler characteristic, count bands |
| `gltf_canonical` | — | parse both files; nodes by name and hierarchy; meshes as multisets of triangles of full corner-attribute tuples; accessors decoded with `tol`; materials as normalised JSON; animations by sampled values; `extensionsUsed` in file order (M34S4-5); ignore `asset.generator` [GLTF §7.2] |
| `image_ssim` | T4 | SSIM ≥ threshold + mean ΔE in a centre crop. vs frap-blessed ≥ 0.995 (gating from M1); Workbench vs Blender ≥ 0.97 (reported in M1, gating from M2); EEVEE vs Blender ≥ 0.90 (gating from M5, raised over time) |
| `gltf_roundtrip` | — | the differential round-trip law of §17.2 on the allow-listed fixture set |

### 20.5 Fixtures, live mode, CI

- `cargo xtask golden bless [--filter <glob>]` runs Blender and writes `tests/golden/fixtures/<area>/<case>.blender.json` (+ `.glb`/`.png`), stamped with Blender version and build hash. Fixtures are committed.
- `cargo test -p frap-golden` runs frap on every case and compares with the committed fixtures, so CI without Blender still compares against Blender truth.
- `FRAP_GOLDEN_LIVE=1` (nightly, and locally when Blender is found) re-runs Blender too and fails on fixture drift. When Blender is missing, live tests are skipped with a visible notice (`#[ignore]` + message); fixture comparisons still run.
- `cargo xtask golden report` prints the operator × tier coverage matrix.
- CI matrix: macOS arm64 (Metal), Linux x86_64 (Vulkan via Mesa lavapipe), Windows x86_64 (DX12 WARP). Plus a clean container with only `cc`/`c++` that builds the default features, `cargo deny`, `cargo xtask codegen --check`, `cargo clippy -D warnings`, `cargo fmt --check`, `cargo xtask fmt --check` (rustfmt on every tracked hand-written `.rs`, including the `automod` modules `cargo fmt` does not reach; generated files are left to their generators).

---

## 21. Conventions

### 21.1 Naming

- Crates `frap-<area>`; ops crates `frap-ops-<namespace group>`.
- Blender identifiers appear **verbatim** in strings and generated code (`"mesh.primitive_cube_add"`, `"MESH_OT_primitive_cube_add"`, `"location"`). Rust types for Blender structs use Blender's struct names (`Object`, `Mesh`, `BMesh`, `BMVert`, `Keyframe`); data fields use RNA identifiers.
- Operator implementation files are named after the operator: `ops/<op>.rs` for the crate's primary namespace (`frap-ops-mesh/src/ops/primitive_cube_add.rs`) and `ops/<namespace>__<op>.rs` for other namespaces in the same crate (`frap-ops-object/src/ops/view3d__snap_cursor_to_center.rs`, `frap-ops-data/src/ops/material__new.rs`, `frap-ops-render/src/ops/render__render.rs`). Primary namespaces: object, transform, mesh, uv, anim, frap; `frap-ops-data`, `frap-ops-io` and `frap-ops-render` have none; RNA function hooks `funcs/<struct>_<func>.rs` (`funcs/vertex_groups_new.rs`); bmesh operators `ops/<bmesh_op>.rs`; mode handlers `frap-ops-object/src/modes/<mode>.rs`; transform mode handlers `frap-ops-transform/src/<mode>/`; glTF mesh extension points `export/mesh/{extract,writers,post}/<name>.rs`; modifiers `types/<type_lowercase>.rs`; glTF extensions `ext/<extension_name_lowercase>.rs`; material patterns `patterns/<feature>.rs`; golden cases `cases/<area>/<op>_<scenario>.py`.
- frap-only: operators in `frap.*`; operator properties prefixed `frap_`; JSON envelope fields outside Blender values; hint codes `UPPER_SNAKE`.

### 21.2 Errors

- Library crates: `thiserror` enums, converted with `From` into `FrapError`. No `unwrap()`/`expect()` on user-controlled data; `expect("invariant: …")` only for internal invariants. Panics are bugs, caught and reported as `INTERNAL`.
- Each crate defines its error codes as `pub const FOO: ErrorCode = ErrorCode("FOO");` in its own `errors.rs` and registers an `ErrorCodeDoc` in the `ERROR_CODES` slice.
- Messages copy Blender wording; frap explanations go in `hint`/`detail`.

### 21.3 Numbers

f32 storage everywhere Blender stores f32; f64 only inside algorithms (QEM, LSCM/ABF/SLIM, plane fits, exact predicates) and where Blender uses double; round back to f32. No fast-math. `mul_add` only where Blender's source uses it. Transcendental functions come from the `libm` crate (rule 2.2.12), so frap's own output is bit-identical across OSes. Bit-identity with Blender is not promised (arm64 FMA contraction, platform libm); goldens use tolerances (T1 1e-5; bevel and UV solvers 1e-4).

### 21.4 Determinism

1. No observable `HashMap`/`HashSet` order: `IndexMap`/`BTreeMap`/sorted vectors wherever data is serialized, iterated into output, used to allocate names or to build geometry. `rustc-hash` maps for lookups only.
2. Session counters (`session_uid`, node uid, modifier `persistent_uid`) start fixed per session; `IdKey` values never appear in output.
3. Randomness only from ports of Blender's generators: `BliRng` (drand48-style LCG) for `select_random`, `vertex_random`, `randomize_transform`, subdivide fractal; `BLI_hash_int_*` for Workbench RANDOM colours. Seeds are operator props. No time- or OS-based randomness.
4. Parallelism (`rayon`) only where results are order-independent or reduced in index order; parallel float sums in fixed chunk order.
5. Byte-stable outputs: `.frap`, GLB and JSON responses are identical across runs and platforms for the same input (CI diffs transcripts on all three OS). This relies on rule 2.2.12 (`libm` for every transcendental in geometry and math: UV sphere, torus, rotations, F-curve easing) and on a deterministic solver configuration (single-threaded or fixed-order reductions) in `faer`.
6. GPU output is the documented exception (vendor differences → tolerances). On one adapter frap is deterministic: fixed TAA sequence, fixed transparency sort, no atomics-order dependence.
7. No wall-clock values in output unless `--timing`.

### 21.5 Logging

`tracing` everywhere; `tracing-subscriber` initialised only in `frap-cli`, writing to **stderr**. Filter via `FRAP_LOG` (EnvFilter syntax, default `warn`) or `--log-level`; `--log-format json` for machine logs. One span per statement (`seq`, `line`, `kind`). Never log scene contents at `info` or above. Library crates never print.

### 21.6 Code

- `#![deny(unsafe_code)]` in every crate (workspace lint). The only allowed exception is `#[allow(unsafe_code)]` on a `linkme` slice declaration or entry, which expands to `#[link_section]` and trips the lint, so `forbid` is impossible in registering crates (RFC 0001 F-LINK-1); `#[operator]`, `#[rna_func]`, `#[rna_hook]`, `#[modifier]` and `#[bmesh_op]` emit that allow themselves, so hand-written code never needs it. `frap-render`, `frap-codec-mesh` and `frap-imbuf` may `allow` real unsafe blocks (wgpu/bytemuck interop, FFI wrappers), each with a `// SAFETY:` comment.
- `cargo fmt`, `cargo clippy --workspace --all-targets -D warnings`. Public items documented (`#![warn(missing_docs)]` on contract crates).
- Every `pub` contract type is `#[non_exhaustive]` where it is an enum likely to grow.

### 21.7 Performance budgets (tracked from M2, enforced in M8)

Cold start < 30 ms without GPU init; a 100k-face edit-mode op < 200 ms; a property read in Edit Mode never triggers modifier re-evaluation (§6.3); 1M-corner GLB export < 2 s; snapshot clone O(#IDs); clean build of each `frap-spec-*` crate within its recorded budget (§4.2).

### 21.8 Process

- RFCs: `docs/design/rfc/NNNN-title.md`, required for any CONTRACT change or new external dependency; merged before dependent work.
- `docs/deviations.md`: every behaviour that differs from Blender, with rationale and golden-side normalisation. Owned by the integrator; WPs add entries via their PR description.
- Commits: one WP per PR; PR title `[M<n>.<wp>] <title>`.

---

## 22. Milestone plan

### 22.1 Rules

- A milestone is a sequence of **waves**. Work packages (WPs) inside one wave run in parallel, each by one agent.
- **Wave 0 of every milestone from M1 on is a single integrator WP.** It is the only WP that edits shared files: root `Cargo.toml`, crate `Cargo.toml`s, `lib.rs` module declarations, `frap-ops-all`, `spec/current_milestone`, and new CONTRACT signatures (bodies `todo!()`). It creates every module file named in that milestone's ownership lists as a compiling stub, and every extension-point slice (§3.4) the milestone's WPs register into. **Exception:** per-concept files inside `automod` directories (`ops/primitive_*.rs`, `funcs/*.rs`, `modes/object.rs`, …) are not stubbed: W0 creates the directory with its `_dir.rs` placeholder and the owning WP adds the files, because the §10.5 registry test requires every concept file to register something (decision log W0-23). It also creates the test-only crates (`crates/frap-ops-<x>-tests`, rule 2.2.11) that a WP's acceptance needs, and the dev-dependencies every WP's tests need. After Wave 0, **a WP edits only the paths it owns**. Directories of per-concept files (`ops/`, `funcs/`, `types/`, `patterns/`, `ext/`, `modes/`, `extract/`, `writers/`, `post/`, `cases/`) are included with `automod::dir!`, so adding a file there needs no `mod` edit.
- **`lib.rs` and module files.** Every crate's `Cargo.toml` and `src/lib.rs` belong to the integrator, so W0 declares every module a WP needs. A WP that needs another module declares it as a submodule of a file it owns (`polyfill.rs` → `polyfill/beautify.rs`), never in `lib.rs`. Per-concept directories are `automod` directories with a `_dir.rs` placeholder until their first file lands (rule 2.2.1).
- **Ownership is per milestone.** A path owned by a WP in one milestone may be re-assigned in a later milestone (stated explicitly as "reassigned"). Where later milestones extend behaviour that one file dispatches (mode switching, transforms, glTF primitive export), the dispatcher is owned once and later WPs add handler files to an extension-point slice instead of reassigning it.
- **Needs** lists the WPs whose *signatures* a WP needs (always satisfied by Wave 0) and, marked *(impl)*, WPs whose implementation must have landed. Within a wave there are no *(impl)* dependencies except where marked *(soft)*: the WP can start against stubs and fixtures, and its acceptance runs once the other lands.
- **Golden prerequisite set.** Golden cases are scripts that both tools run, so a WP's goldens need the operators and RNA functions they call. Each milestone names a *golden prerequisite set* (the minimal calls its goldens use to build a scene: primitives, selection, mode switching, data-API creation). It is owned by WPs that land **one wave earlier** than every WP whose goldens use it, and nobody's acceptance depends on a same-wave prerequisite.
- **Acceptance follows ownership.** A WP is accepted only on behaviour implemented in files it owns.
- **Every WP ships its own tests**: unit tests, golden cases under its own `tests/golden/cases/<area>/<topic>/`, CLI transcripts under its own `tests/cli/<milestone>/<topic>/`, and `--describe` snapshots per operator (`crates/frap-schema/tests/snapshots/op__<idname>.snap`, one file per op, created by the WP that implements the op).
- A WP is done when its acceptance tests pass on the CI matrix (macOS arm64, Linux + lavapipe, Windows + WARP).
- Operator-by-operator milestone assignment is in `docs/OPERATORS.md`; its `M` column also drives which operators get typed props structs (§4.2).

### 22.2 M0: Walking skeleton

**Goal**: run one statement of each kind end to end through the real crate boundaries, on all three OSes, before any contract is frozen. M0 is exempt from the Wave 0 rule: two WPs with disjoint paths run in parallel. Its code is kept: M1 W0 grows it into the full crate tree.

**Exit transcript** (`tests/cli/m0/skeleton.trycmd`):

```
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))
mesh.primitive_cube_add size=2 location=0,0,1
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.data.objects['Cube'].location
bpy.ops.export_scene.gltf(filepath='out/m0.glb')
```

| WP | Title | Owns | Needs | Acceptance |
|---|---|---|---|---|
| 0.1 | Walking skeleton | root `Cargo.toml`, `rust-toolchain.toml`, `.github/workflows/ci.yml`, minimal `crates/{frap-rna,frap-lang,frap-mesh,frap-scene,frap-op,frap-session,frap-io-gltf,frap-cli}/**`, `spec/m0/**` (hand-written `OperatorSpec` for `mesh.primitive_cube_add` and `StructDef` for `Object.location`), `tests/cli/m0/**` | — | The exit transcript produces one JSON response per statement (op, op, set, get, op) with the §13.2 envelope; both cube statements lower to the same IR; the GLB passes `gltf =1.4.1` and imports in Blender 5.2 (skipped without Blender); a plain dependency graph obeys the §3.2 layers; green on macOS, Linux, Windows. |
| 0.2 | Contract probes | `crates/frap-probe/**` (scratch crate, deleted in M1 W0), first versions of `tools/blender/dump_ops.py` and `tools/blender/dump_rna.py` (namespaces `mesh`, `object`; structs `Object`, `Mesh`, `MeshVertex`, `Material`, `ShaderNodeTree`, `NodeSocket`), `.github/workflows/gpu-probe.yml`, `docs/design/rfc/0001-m0-contract-findings.md` | — | `PropDef`/`StructDef`/`OperatorSpec` are generated from the real dumps of that subset without hand edits; prototypes of `MeshVertexMut` (SoA proxy), derived `MaterialSlot`, an embedded node-tree path (`Root::Embedded`, repr string of Appendix A), `Context`, a `Box<dyn RenderService>` reached through `OpCtx::services()`, and `OpCtx::split_edit` compile against the §9.4/§10.1 signatures with get/set tests; a wgpu clear-and-readback test passes on Metal, lavapipe and WARP; RFC 0001 lists every contract change found. |

M1 W0 starts only after RFC 0001 is merged.

### 22.3 M1: Thin vertical slice

**Shippable**: `frap` reads both syntaxes from stdin and answers in JSON, plus `frap serve --jsonl` and `frap mcp`; all `mesh.primitive_*`; object selection, add, delete, duplicate, join, parenting, origin, apply, clear, shading; `transform.translate/rotate/resize` in object mode; typed get/set of every implemented property; bulk `foreach_get`/`foreach_set`; `bpy.ops.<op>.poll()`; data API (`bpy.data.*.new/remove`, links, slots); basic materials (Principled BSDF base colour, metallic, roughness, plus an Image Texture on base colour); static-mesh GLB export and import; `.frap` save/open; Workbench render to PNG; `--describe` listing every 5.2 operator in the curated namespaces; golden harness with the first cases. `render.render` with the factory engine (`BLENDER_EEVEE`) renders with Workbench and a WARNING until M5. `enter_editmode=True` and `mode_set(mode='EDIT')` return `OP_NOT_IMPLEMENTED` (no mode handler) until M2.

**Golden prerequisite set (M1)**: all `mesh.primitive_*_add`, `object.select_all`, `object.delete`, `object.mode_set(mode='OBJECT')` (WP 1.19, W3); `Object.select_set/select_get`, `view_layer.objects.active`, `Object.parent` assignment (WP 1.10, W2); `bpy.data.<coll>.new/remove`, `mesh.materials.append`, `nodes.new`, `links.new` (WP 1.22, W3). W4 goldens may use all of these; W3 goldens use only the factory scene, the W2 RNA functions and their own WP's calls.

**Exit transcript** (`tests/cli/m1/vertical_slice.trycmd`, green on all three OS; cwd-relative output paths, no `//`, §17.2):

```
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete()
mesh.primitive_cylinder_add vertices=16 radius=0.5 depth=2 location=0,0,1
bpy.data.materials.new("Steel")
bpy.data.materials['Steel'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Steel'].diffuse_color = (0.6, 0.6, 0.7, 1.0)
bpy.context.object.data.materials.append(bpy.data.materials['Steel'])
bpy.ops.object.camera_add(location=(4, -4, 3), rotation=(1.1, 0, 0.785))
bpy.context.scene.camera = bpy.data.objects['Camera']
data.scenes['Scene'].render.engine = BLENDER_WORKBENCH
bpy.context.scene.render.filepath = 'out/part.png'
bpy.ops.render.render(write_still=True)
bpy.ops.export_scene.gltf(filepath='out/part.glb')
bpy.ops.wm.save_as_mainfile(filepath='out/part.frap')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/part.glb')
bpy.data.objects['Cylinder'].data.materials[0].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value
bpy.ops.wm.open_mainfile(filepath='out/part.frap')
bpy.data.objects['Cylinder'].location
```

Plus: ≥ 60 golden cases blessed and green (primitives at T1, object ops, RNA get/set T0, error classes T0, ≥ 10 static GLB cases), ≥ 3 Workbench renders green against frap-blessed self-goldens (SSIM ≥ 0.995; SSIM vs Blender Workbench is reported, and gates from M2 WP 2.3), `.frap` round-trip lossless and byte-identical, the same transcript green through `frap serve --jsonl`.

| WP | Wave | Title | Owns | Needs | Acceptance |
|---|---|---|---|---|---|
| 1.0 | W0 | Skeleton and contracts (integrator) | root `Cargo.toml`, `rust-toolchain.toml`, `deny.toml`, `.github/workflows/**`, `xtask/Cargo.toml`, `xtask/src/main.rs`, `xtask/src/check_deps.rs`, `xtask/src/budgets.rs`, `xtask/budgets.toml`, `spec/current_milestone`, every `crates/*/Cargo.toml` and `src/lib.rs` (all crates of §3.2 created, later-milestone crates empty), every CONTRACT file of §3.4 frozen at M1 W0 as stubs (incl. the `MODE_HANDLERS`, `TRANSFORM_MODE_HANDLERS` and glTF `PRIMITIVE_*` slices), `crates/frap-ops-all/**`, `docs/deviations.md`, `fuzz/Cargo.toml`, `crates/*/clippy.toml`, `crates/frap-schema/tests/describe_ops.rs` (stub; then WP 1.18) | M0, RFC 0001 | Folds M0 code in, deletes `crates/frap-probe`; `cargo build/test --workspace` green on the CI matrix; `cargo deny check` and `cargo xtask check-deps` (no dev-dependency cycles) pass; the clean `cc`-only container builds default features; every CONTRACT signature in this document compiles. |
| 1.1 | W1 | Blender dumps and codegen | `tools/blender/dump_{ops,rna,nodes,modifiers,startup}.py`, `spec/**` (except `spec/blender-5.2/primitives/`, `spec/current_milestone`), `xtask/src/codegen/**`, the `[baseline_secs]` values of `xtask/budgets.toml` (written by `cargo xtask budgets --record`), appended `[dependencies]` lines of `xtask/Cargo.toml` (W0-28), `crates/frap-spec-rna/**`, `crates/frap-spec-ops-*/**`, `crates/frap-spec-catalog/**`, every `crates/*/src/generated/**` | 1.0 | `cargo xtask codegen --check` clean; drift tests pass; typed props structs exist exactly for OPERATORS.md operators with M ≤ 1, every other curated operator decodes from the catalog with a stable `&'static` address; `PrimitiveCubeAdd` equals §10.4; plain structs' `Default` equals dumped fresh values (test per struct); dumps taken with the glTF variants preference on (§4.1); `frap_*` option audit test (§4.2); `frap-spec-*` build-time budgets recorded; overlay files seeded (`dynamic_enums`, `python_only`, `compat_4x`, `frap_options`). |
| 1.2 | W1 | Math, RNA core and RNA macros | `crates/frap-math/**`, `crates/frap-rna/**`, `crates/frap-macros/src/{rna_struct,rna_func,rna_hook}.rs`, `tools/blender/dump_rng.py` | 1.0 | `coerce` table tests cover every row of [RNA §1.2] with the messages of Appendix A (an empty message body drops the prefix's trailing space: `TypeError: bpy_struct: item.attr = val:`); `clamp_report` wording of §9.2 tested for scalar min/max and arrays; `unique_name` reproduces all verified cases; `BliRng` reproduces a Blender-dumped sequence; Euler/matrix decomposition within 1e-6 of Blender, bit-identical across the 3 OSes (`libm`); `IdKind` markers for every `IdCode`; `Root::Embedded` repr/path_from_id reproduce the verified socket strings; `serde_buf` round-trip; `rna_struct!` generates value, hook, coll and SoA-proxy bindings for test structs; `check_bindings` and `HOOK_DECLS` emission. |
| 1.3 | W1 | Language front-ends | `crates/frap-lang/**`, `fuzz/fuzz_targets/lang_*.rs` | 1.0 | Corpus ≥ 250 statements (one file each) incl. every example in this document with IR snapshots; the four pairs of §11.5 produce identical IR; `parse_complete` never waits (unclosed bracket → `SyntaxError: '(' was never closed`); raw splitter ends continuations on blank line/EOF; §11.4 path/BARE and `D`/`C` routing cases; `bpy.ops.x.y.poll()` → `OpMeta`; every error has a span with byte `col` and `col_chars` (non-ASCII test); proptest/fuzz 10 min without panic. |
| 1.4 | W1 | Mesh, geometry, primitives | `crates/frap-mesh/**` (except `src/rna/**`), `crates/frap-geom/**`, `tools/blender/dump_primitives.py`, `spec/blender-5.2/primitives/**` | 1.0 | All 10 primitive generators index-exact (T1) vs dumps for defaults and ≥ 2 variants each (monkey per D1), bit-identical across OSes; `normals_domain` rules; looptri rule; default cube corner split = 24; generators create Blender's internal `.select_vert/.select_edge/.select_poly` and `.uv_select_vert/.uv_select_edge/.uv_select_face` attributes (closes the deviations.md §2 gap); all-false attribute removal; `from_pydata` (Rust API); `MeshRuntime` clone is empty and every `&mut` accessor clears caches (proptest: cached normals/looptris always equal a fresh computation). |
| 1.5 | W1 | glTF format crate | `crates/frap-gltf/**` (core JSON, `glb.rs`, `accessor.rs`, `builder.rs`, `validate.rs`, `ext/khr_lights_punctual.rs`, `ext/khr_texture_transform.rs`, `ext/khr_materials_emissive_strength.rs`), `tests/data/gltf-samples/**` | 1.0 | Reads a vendored Khronos sample subset (licences recorded) and writes it back losslessly modulo buffer layout; all primitive modes (POINTS…TRIANGLE_FAN) decode; unknown extensions/extras round-trip; `gltf =1.4.1` reads frap output; validator tests. |
| 1.6 | W1 | Images and colour | `crates/frap-imbuf/**`, `crates/frap-color/**`, `tools/blender/bake_view_luts.py`, `assets/luts/**` | 1.0 | PNG 8/16, JPEG, WebP lossless, HDR, EXR decode/encode round-trips; Standard and AgX within ΔE < 1 of Blender on the **held-out** colour set, not the baked lattice (CPU test). |
| 1.7 | W1 | GPU and basic Workbench | `crates/frap-render/**`, `assets/studio_lights/**` | 1.0 | `new_service` works on Metal, lavapipe, WARP (Windows: Vulkan, else DX12 with FXC; no `static-dxc`); one Workbench uber-shader, no per-material pipelines; synthetic `RenderScene`s render with SOLID + STUDIO/FLAT + MATERIAL/OBJECT/SINGLE/RANDOM, `film_transparent`, background; `Rgba32Float` accumulation; readback padding test; same-adapter determinism (SSIM ≥ 0.995 between runs); cold first-render time recorded per CI adapter. |
| 1.8 | W1 | Golden harness | `crates/frap-golden/**`, `tools/blender/{golden_runner,dump_state,emulations,test_golden_runner}.py`, `tests/golden/props.toml`, `tests/golden/README.md`, `tests/golden/{cases,fixtures}/harness/**` (decision log W1-14), `xtask/src/golden.rs`, appended `[dependencies]` lines of `xtask/Cargo.toml` (W0-28) | 1.0 | Comparators `lines`, `rna_values`, `mesh_exact`, `mesh_canonical`, `gltf_canonical`, `gltf_roundtrip`, `image_ssim` with self-tests (equal under permutation, different under a one-element change); runner normalisation list matches `deviations.md` (no time normalisation, §20.3); bless/live/skip paths tested; `locate_blender` order tested; Blender-side runner blesses 3 seed cases. |
| 1.9 | W1 | `.frap` container | `crates/frap-project/src/{container,manifest,buffers,migrate}/**` | 1.0, 1.2 *(soft)* | Deterministic zip (byte-identical twice); content addressing and inline threshold; migration framework with a v0→v1 test; refuses newer versions. |
| 1.10 | W2 | Scene data, startup and scene RNA functions | `crates/frap-scene/**` (except `src/generated/**` and `src/lib.rs`; the datablock field sets in `src/id/` are frozen by W0, W0-21) | 1.1 *(impl)*, 1.2 *(impl)* | `IdMap` COW/generation/`ChangeLog` tests; ordering rule (§5.4) test; `(name, library)` keys, local-before-linked lookup and the tuple-key rule pinned by T0 goldens; `Library` ID; embedded IDs mutate through the owner (owner `generation` bump); rename/remove/users/orphans; derived material slots (count follows `data.materials`, overrides padded lazily); factory and empty startup equal `startup.json` (compared through RNA), including the factory `World` (closes the deviations.md §2 gap); bindings for Object, Scene, ViewLayer, LayerCollection, Collection, Camera, Light (all types), World, Material (non-node), Image metadata, Window stub, generated settings structs pass `check_bindings`; hooks `rotation_mode` conversion and `matrix_world` write decomposition (goldens vs Blender), `parent`, `name`, `frame_current` (marks time dirty only); RNA functions `select_set/select_get/hide_set/hide_get/visible_get`, `view_layer.objects.active`; `Context` members; Python-only `children`, `children_recursive`, `users_collection`, `users_scene`. |
| 1.11 | W2 | Nodes and basic lowering | `crates/frap-nodes/**` (except `src/generated/**`), `crates/frap-material/**` | 1.1 *(impl)*, 1.2 *(impl)* | `materials.new` graph and locations; embedded node tree (`is_embedded_data`, verified repr); first-enabled socket lookup (Mix test); `links.new` replacement; `ShaderNodeSeparateRGB` error; patterns `base_color`, `metallic_roughness`, constant `alpha`, constant `emission`, Image Texture on base colour; `lower(raise(lower(x))) == lower(x)` on M1 fixtures; every `patterns/*.rs` registers exactly one `MATERIAL_PATTERNS` entry and `lower`/`raise` name no pattern. |
| 1.12 | W2 | Operator framework | `crates/frap-macros/src/{operator,modifier,bmesh_op}.rs`, `crates/frap-op/**` | 1.2 *(impl)* | Dummy ops in integration tests under `crates/frap-op/tests/` (each file is its own test binary, so a duplicate-idname test does not disturb the others) register via `linkme`; duplicate idnames make `Registry::global()` fail; pipeline tests for unknown op + suggestions, a catalog operator without an implementation (`mesh.bevel`) → `NotImplementedError` `OP_NOT_IMPLEMENTED` (closes the deviations.md §2 gap), unknown keyword wording, positional rule, macro args in both spellings, dynamic enums, `is_set`, clamping report (exact §9.2 wording), `ANGLE_LIKELY_DEGREES` hint; declarative poll: `requires`/`unmet`/`hint` for every poll in the library; `OpMeta` (`poll()`, `get_rna_type()`, `idname()`, `idname_py()`); `split_edit`/`split_eval`/`services` borrow tests; `placement` matches Blender for `align='VIEW'` and cursor cases; `add_primitive_or_merge`/`add_object_with_data` uniquify object and data independently; `frame_range_or_scene` and `iter_edit_meshes` implemented and unit-tested (no `todo!()` left in `frap-op`). |
| 1.13 | W2 | Evaluation v1 | `crates/frap-eval/**` | 1.10 *(soft)* | World matrices with parents and `matrix_parent_inverse`; bounds and `dimensions`; fingerprint invalidation on every `gen` bump; reads never change a fingerprint; cycle report; evaluated mesh shares the original `Arc` when there are no modifiers; `apply_pending_animation` boundary API (no-op until M6). |
| 1.14 | W2 | Session and protocol | `crates/frap-session/**` | 1.3 *(impl)*, 1.10 *(soft)*, 1.12 *(soft)* | Every path form of [RNA §1.1]; get/set/call/print/op_meta responses and value encoding (§13.3) incl. nameless `$coll`, `$array`, `foreach_get`/`foreach_set`, slices and `--max-array` truncation; `changes` and `context`; reads run without a transaction and leave `State` bit-identical; rollback on ERROR/CANCELLED/panic with a proptest that state is unchanged; `exec_request` with `id` echo, done record (`DoneRecord::from_responses`) and `on_error` (`SKIPPED`); `Services` with a lazily created `RenderService` (stub when `gpu` is off); compat errors with hints. |
| 1.15 | W2 | Render-scene builder | `crates/frap-render-scene/src/build/**`, `tools/blender/dump_camera_cases.py` | 1.10, 1.11, 1.13 *(soft)* | Camera matrices equal Blender's for sensor fit/shift/ortho/clip cases; deterministic object order; mesh extraction equals `corner_split`; Workbench colour sources; builds from a `State` clone without touching the session state. |
| 1.16 | W2 | glTF static export (library level) | `crates/frap-io-gltf/src/export/**` (incl. `mesh/primitives.rs`, `mesh/extract/triangles.rs`, `mesh/writers/{normal,texcoord}.rs`), `crates/frap-io-gltf/tests/{export_*.rs,export_support/**,snapshots/export_*}` (W2-11) | 1.5, 1.10, 1.11 *(soft)* | Rust-built scenes export to GLBs that pass `validate.rs` and `gltf =1.4.1`; unit tests of the Blender rules (looptris, corner split, 4-dp normals, V flip, natural root order, Y-up, omission rules, BYTE_COLOR → USHORT); the three extension-point slices drive TRIANGLES/NORMAL/TEXCOORD_0 (a test-only writer proves a new file is picked up); `insta` snapshots. Blender goldens are in 1.23. |
| 1.17 | W2 | glTF static import (library level) | `crates/frap-io-gltf/src/import/**`, `tools/blender/make_gltf_fixtures.py`, `tests/data/gltf-blender/**`, `crates/frap-io-gltf/tests/import_*.rs` (W2-11) | 1.5, 1.10, 1.11 *(soft)* | For ≥ 5 Blender-exported fixture GLBs and the Khronos subset, calling the importer in-process yields a state equal to Blender's `import_scene.gltf` dump, compared field by field against the dump JSON (`frap-io-gltf` cannot dev-depend on `frap-golden`, rule 2.2.11; the `GoldenState` comparison runs through the operator in 1.23) (names, hierarchy, `merge_vertices`, `import_shading`, strips/fans/lines/points converted). No operators needed. |
| 1.18 | W2 | Schema export | `crates/frap-schema/**`, `docs/agents.md` | 1.1, 1.12 | Manifest, jsonschema and tools snapshots; tool formats validate against the provider JSON-Schema subsets; `poll.requires` per operator; angle units shown as `ROTATION (radians)`; unimplemented ops visible with `not_implemented`; `agents.md` covers `serve`, `mcp`, `on_error`, `poll()`, bulk reads. |
| 1.27 | W2 | Mesh RNA bindings | `crates/frap-mesh/src/rna/**` | 1.1 *(impl)*, 1.2 *(impl)*, 1.4 *(impl)* | `Mesh` and element bindings (`MeshVertex`, `MeshEdge`, `MeshPolygon`, `MeshLoop`, `MeshLoopTriangle`, `MeshUVLoopLayer`/`MeshUVLoop`, attributes and typed `data[i]`) through SoA proxies pass `check_bindings`; `use_smooth`/`use_seam`/`use_edge_sharp` proxies create/remove attributes; `foreach_get`/`foreach_set` overrides on every SoA collection. (The `from_pydata` RNA function moved to 1.22, decision log W0-22.) |
| 1.19 | W3 | Primitives and object golden prerequisites | `crates/frap-ops-mesh/src/ops/primitive_*.rs`, `crates/frap-ops-object/src/ops/{select_all,delete,mode_set}.rs`, `crates/frap-ops-object/src/modes/object.rs`, `tests/golden/cases/primitives/**`, `tests/golden/cases/object/delete/**` | 1.4, 1.12, 1.14 *(impl)*, 1.10 *(impl)* | All 10 `mesh.primitive_*_add` at T1 incl. `location`/`rotation`/`align`/`scale` variants and cursor placement; `cube_add_after_delete` (object `Cube`, mesh `Cube.001`) at T0; `select_all` actions; `delete` leaves orphans; `mode_set(mode='OBJECT')` and `OP_NOT_IMPLEMENTED` for modes without a handler. |
| 1.22 | W3 | Data API core | `crates/frap-ops-data/src/funcs/**`, `tests/golden/cases/data/**` | 1.10, 1.11, 1.14 *(impl)*, 1.27 *(impl)* | `bpy.data.<coll>.new/remove/get/find` for objects, meshes, materials, cameras, lights, collections, worlds, images (`new`); `collection.objects.link/unlink`, `children.link`; `mesh.materials.append/pop/clear` (slots follow on every user object); `nodes.new/remove`, `links.new/remove`; `evaluated_get`, `evaluated_depsgraph_get`, `to_mesh`, `view_layer.update`; `ID.rename/copy/user_clear`; the Python-only `Mesh.from_pydata/shade_smooth/shade_flat` RNA functions (`funcs/mesh_*.rs` over the WP 1.4 Rust API; moved from 1.27, decision log W0-22). Goldens use only the factory scene and data-API calls. |
| 1.25 | W3 | CLI, serve and MCP | `crates/frap-cli/**`, `tests/cli/m1/cli/**` | 1.14 *(impl)*, 1.18 *(impl)* | Flags (§13.1, §14); exit codes; `--fail-fast`; ready/end events; raw mode never hangs on an unclosed bracket (blank line/EOF → `SyntaxError`); `frap serve --jsonl` transcripts (ids, done records, `on_error`); `frap mcp` handshake, `tools/list` equals `--describe=tools-anthropic`, `tools/call` keeps state across calls. |
| 1.20 | W4 | Object ops: select, add, duplicate, join, collections, slots, material.new | `crates/frap-ops-object/src/ops/{select_by_type,select_pattern,camera_add,light_add,empty_add,duplicate,duplicate_move,duplicate_move_linked,join,move_to_collection,link_to_collection}.rs`, `crates/frap-ops-object/src/ops/{collection_*,material_slot_*}.rs`, `crates/frap-ops-data/src/ops/material__new.rs`, `tests/golden/cases/object/{select,add,duplicate,join,collection,slots}/**`, `tests/cli/m1/object_ops/{add_duplicate_join,duplicate_move}.*` (W4-1) | 1.12, 1.19 *(impl)*, 1.22 *(impl)* | Selection base order; duplicate preferences (materials shared); join CANCELLED reports; `camera_add` sets `scene.camera` when None; camera/light data named independently of the object; material slot ops on shared meshes. |
| 1.21 | W4 | Object ops: transforms, parenting, origin, apply, shading, snapping | `crates/frap-ops-transform/src/{ops/translate.rs,ops/rotate.rs,ops/resize.rs,core/**,object/**}`, `crates/frap-ops-object/src/ops/{transform_apply,parent_set,parent_clear,parent_no_inverse_set,origin_set,location_clear,rotation_clear,scale_clear,shade_smooth,shade_flat,shade_smooth_by_angle}.rs`, `crates/frap-ops-object/src/support/**`, `crates/frap-ops-object/src/ops/view3d__snap_*.rs`, `tests/golden/cases/object/{transform,parent,origin,apply,shade,snap}/**`, `tests/cli/m1/object_ops/transform_parent_apply.*` (W4-2) | 1.12, 1.13, 1.14 *(impl)*, 1.19 *(impl)* | `transform.*` with orient types and pivots in object mode through `TRANSFORM_MODE_HANDLERS` (matrix decomposition loses shear like Blender); `parent_set` inverse; `transform_apply` incl. multi-user error; all `origin_set` types; shade ops' attribute effects; snap ops. |
| 1.23 | W4 | IO operators, `.frap` mapping, glTF goldens | `crates/frap-ops-io/**`, `crates/frap-ops-io-tests/**` (test-only crate with a `Session`, rule 2.2.11), `crates/frap-project/src/scene_map/**`, `tests/golden/cases/gltf/m1/**`, `tests/golden/cases/io/**`, `tests/data/gltf-roundtrip/allow.toml` | 1.9, 1.16, 1.17, 1.10 *(impl)*, 1.19 *(impl)*, 1.22 *(impl)* | `gltf_canonical` equal to Blender on ≥ 10 export cases: primitives, hierarchy (via `Object.parent`), multi-material, transforms, base colour/metallic/roughness + base-colour texture, flat vs smooth normals, `use_selection`, `export_format` GLB and GLTF_SEPARATE; ≥ 5 import goldens through the operator; differential round-trip law (§17.2) on the M1 allow-list; `wm.save_as_mainfile/save_mainfile/open_mainfile/read_homefile/read_factory_settings`; proptest (in `frap-ops-io-tests`): random M1 op sequences → save → open → identical state; byte-identical saves; orphans dropped; linked built-in groups and window state round-trip; all 110/20 glTF props accepted (unsupported non-default values give a WARNING naming the prop); `//` resolution as listed in `deviations.md`. |
| 1.24 | W4 | Render operator | `crates/frap-ops-render/src/ops/render__render.rs`, `crates/frap-ops-render/src/output.rs` (+ `output/**`), `crates/frap-ops-render-tests/**` (test-only crate, rule 2.2.11, W4-4), `tests/golden/cases/render/m1/**`, `tests/golden/fixtures/render/m1/**` | 1.6, 1.7, 1.15 *(impl)*, 1.19 *(impl)* | ≥ 3 Workbench scenes match frap-blessed self-goldens (SSIM ≥ 0.995) on every CI adapter; SSIM vs Blender Workbench reported (not gating); GPU reached only via `services().render()`; `GPU_UNAVAILABLE` without `gpu`; output path rules (cwd-relative and `//`); `Cannot render, no camera`; `Render Result` image; EEVEE/CYCLES fall back to Workbench with WARNING; rendering changes nothing in the session except adding/updating `Render Result` (animation evaluated on a clone). |
| 1.26 | W4 | Cross-cutting goldens and exit transcript | `tests/golden/cases/{rna,errors,context,poll}/**`, `tests/cli/m1/vertical_slice*` (the `.trycmd`, the raw-stdin and serve `.toml` cases and their `.in`/`.out`/`.stdin`/`.stdout`, W4-5) | 1.8, 1.14 *(impl)*, 1.19 *(impl)*, 1.22 *(impl)*, 1.20/1.23/1.24 *(soft)* | ≥ 25 T0 cases: every error code reachable in M1 with Blender's class and message; context read-only; path forms; `bpy.ops.<op>.poll()` results equal Blender's; the exit transcript in raw and `serve` modes on all three OS. |

Maximum parallelism: W1 = 9, W2 = 10, W3 = 3, W4 = 5.

### 22.4 M2: Edit-mode modelling, Workbench completion, contact sheet

**Shippable**: Edit Mode on BMesh with multi-object edit, all P0/P1 edit-mode operators of [MESH §4], edit-mode transforms, selection by index for agents, full Workbench (matcap, x-ray, wireframe, outline, shadows), `frap.render_contact_sheet`. **Exit**: the [MESH §5.8] P0 list at its declared tiers (restricted to the edit-mode mesh operators: its UV entries `uv.smart_project`, `uv.unwrap` and `uv.pack_islands` are M3, WP 3.8, and `object.shade_auto_smooth` is M4, as OPERATORS.md plans them; decision log M2S1-1); ≥ 150 edit-mode goldens; Workbench SSIM ≥ 0.97 vs Blender on the M1 render scenes (gate moved from M1); contact-sheet transcript.

**Golden prerequisite set (M2)**: `object.mode_set(mode='EDIT')` and its mode handler, `mesh.select_all`, `mesh.select_mode`, `frap.select_elements` (WP 2.2, W1). Every W2 golden builds its selection with these.

| WP | Wave | Title | Owns | Needs | Acceptance |
|---|---|---|---|---|---|
| 2.0 | W0 | Contract delta (integrator) | `crates/frap-bmesh/src/lib.rs` + public API stubs (§6.2), module stubs for every file below, `frap-ops-all`, crate manifests, `spec/current_milestone`, `spec/frap/ops/frap.<op>.json` seeds, codegen refresh, the M1 leftovers of decision log M2W0-1..4 | M1 | Workspace compiles with every M2 file stubbed; contract pins in `crates/frap-ops-all/tests/contracts.rs`. |
| 2.1 | W1 | BMesh core | `crates/frap-bmesh/src/{types,pool,kernel,query,customdata,select,walkers,convert,validate}.rs`, `crates/frap-bmesh/src/ops/{delete,duplicate}.rs` (moved from 2.7/2.10: extrude, split, separate, spin and solidify build on them, M2W0-6) | 2.0 | Pool order equals [MESH App. B.6]; `from_mesh`→`to_mesh` is the identity (T1) on every primitive; 10k random kernel sequences keep `validate` green. |
| 2.2 | W1 | Edit-mode integration and golden prerequisites | `crates/frap-ops-object/src/modes/edit_mesh.rs`, `crates/frap-ops-object/src/ops/editmode_toggle.rs`, `crates/frap-session/src/edit/**`, `crates/frap-op/src/helpers/{edit_merge,edit_mode}.rs`, `crates/frap-ops-mesh/src/ops/{select_all,select_mode}.rs`, `crates/frap-ops-frap/src/ops/select_elements.rs`, `spec/frap/ops/frap.select_elements.json`, `tests/golden/cases/edit/prereq/**` (W0 wired the session, `.frap` load and `add_primitive_or_merge` to these files, M2W0-5) | 2.0, 2.1 *(soft)* | Multi-object edit through the `MODE_HANDLERS` entry (no edit of `mode_set.rs`); edit-mode reads through the cached view: no `gen` bump, `is_dirty` and eval fingerprints unchanged, one conversion per BMesh generation; `RNA_EDITMODE_WRITE`; primitives in Edit Mode merge and select only new geometry (T1); `select_all` actions, `select_mode` conversion table [MESH §3.3], `frap.select_elements` by index; `edit_mesh` response block; `.frap` re-enters Edit Mode on load. |
| 2.3 | W1 | Workbench completion and contact sheet | `crates/frap-render/src/workbench/{matcap,xray,wire,outline,shadow,cavity}.rs`, `crates/frap-render/src/composite/**`, `crates/frap-ops-render/src/ops/frap__render_contact_sheet.rs`, `assets/{matcaps,font}/**`, `tests/golden/cases/render/m2/**`; reassigned (M2W0-7): `crates/frap-render/src/{engine,service}.rs`, `crates/frap-render/src/workbench/{mod.rs,workbench.wgsl,color.rs,studio_light.rs}`, `crates/frap-render-scene/src/build/{mod.rs,workbench.rs}`, `crates/frap-ops-render-tests/tests/**`; new: `crates/frap-session/src/resolve/context/ui.rs` (OPERATORS.md "Context (preferences, space_data stub)"), `spec/frap/ops/frap.render_contact_sheet.json` | 2.0 | M1 render scenes SSIM ≥ 0.97 vs Blender Workbench (the gate moved from M1); x-ray, wireframe, matcap, outline, shadows SSIM ≥ 0.95 vs Blender; contact-sheet tiles equal individual renders; layout snapshot; scene unchanged after the op. |
| 2.4 | W2 | Selection | `crates/frap-ops-mesh/src/ops/{select_*,loop_select,edgering_select,edges_select_sharp,faces_select_linked_flat,hide,reveal,loop_to_region,region_to_loop,shortest_path_select}.rs`, `crates/frap-bmesh/src/ops/{region_extend,similar}.rs`, `crates/frap-ops-frap/src/ops/{select_by_normal,scene_summary}.rs`, `crates/frap-ops-object/src/ops/material_slot_{select,deselect}.rs`, `spec/frap/ops/frap.{select_by_normal,scene_summary}.json`, `tests/golden/cases/edit/select/**` (all `select_*` files except `select_all`/`select_mode`, owned by 2.2) | 2.1, 2.2 *(impl)* | `select_more/less/linked/random/…`; `select_random` equal per seed; pick ops by index; history/active element. |
| 2.5 | W2 | Extrude and inset | `crates/frap-bmesh/src/ops/{extrude_face_region,extrude_discrete_faces,extrude_edge_only,extrude_vert_indiv,inset_region,inset_individual}.rs`, `crates/frap-ops-mesh/src/ops/{extrude_*,inset}.rs`, `tests/golden/cases/edit/extrude/**` | 2.1, 2.2 *(impl)*, 2.12 *(soft: the macros' transform step, M2W2-6)* | Region/edge/vert extrude and inset at T1 incl. selection after; macros in both spellings. |
| 2.6 | W2 | Subdivide and loops | `crates/frap-bmesh/src/ops/{subdivide_edges,subdivide_edgering,unsubdivide,offset_edgeloops}.rs`, `crates/frap-ops-mesh/src/ops/{subdivide,subdivide_edgering,unsubdivide,loopcut,loopcut_slide,offset_edge_loops,offset_edge_loops_slide}.rs`, `tests/golden/cases/edit/subdivide/**` | 2.1, 2.2 *(impl)* | Uniform subdivide T1 (incl. reversed face-centre order); loopcut by `edge_index` equals the Blender emulation [MESH App. B.7]. |
| 2.7 | W2 | Delete, dissolve, merge | `crates/frap-bmesh/src/ops/{dissolve_verts,dissolve_edges,dissolve_faces,dissolve_limit,dissolve_degenerate,collapse,pointmerge,find_doubles,weld_verts}.rs`, `crates/frap-ops-mesh/src/ops/{delete,delete_loose,delete_edgeloop,dissolve_*,edge_collapse,merge,remove_doubles}.rs`, `tests/golden/cases/edit/delete/**` | 2.1, 2.2 *(impl)* | Delete types T1; merge types (incl. dynamic FIRST/LAST) T1; `Removed N vertice(s)` report and `result.removed`. |
| 2.8 | W2 | Create and fill | `crates/frap-bmesh/src/ops/{contextual_create,triangle_fill,grid_fill,holes_fill,bridge_loops,connect_verts,connect_vert_pair,beautify_fill,rotate_edges,bisect_plane,convex_hull}.rs`, `crates/frap-ops-mesh/src/ops/{edge_face_add,fill,fill_grid,fill_holes,beautify_fill,bridge_edge_loops,vert_connect*,edge_rotate,bisect,convex_hull,face_split_by_edges}.rs`, `tests/golden/cases/edit/create/**` | 2.1, 2.2 *(impl)*, 2.6 *(soft: `bridge_loops` with cuts uses `subdivide_edgering`)*, 2.7/2.9/2.10 *(soft: `dissolve_faces`, `weld_verts`, `reverse_faces`, `split`, `triangulate`, `join_triangles`, M2W2-8)* | Counts and canonical geometry (T2) for every op; error texts (`Connecting edge loops overlap`). |
| 2.9 | W2 | Normals, marks, attributes | `crates/frap-bmesh/src/ops/{recalc_face_normals,reverse_faces}.rs`, `crates/frap-ops-mesh/src/ops/{flip_normals,normals_make_consistent,faces_shade_*,mark_seam,mark_sharp,set_sharpness_by_angle,set_normals_from_faces,customdata_custom_splitnormals_*,attribute_set,uv_texture_add,uv_texture_remove}.rs`, `crates/frap-ops-mesh/src/ops/geometry__*.rs`, `crates/frap-ops-object/src/ops/material_slot_assign.rs`, `crates/frap-ops-data/src/funcs/{mesh_set_sharp_from_angle,mesh_flip_normals,mesh_split_faces,u_v_loop_layers_remove,loop_colors_new}.rs`, `tests/golden/cases/edit/normals/**` | 2.1, 2.2 *(impl)* | Attribute presence/removal rules [MESH §2.5]; `geometry.color_attribute_add` defaults; flip keeps `l_first`. |
| 2.10 | W2 | Split, separate, duplicate, triangulate, misc | `crates/frap-bmesh/src/ops/{split,split_edges,triangulate,join_triangles,poke,spin,solidify,wireframe,symmetrize,smooth_vert}.rs`, `crates/frap-ops-mesh/src/ops/{split,edge_split,separate,duplicate,duplicate_move,quads_convert_to_tris,tris_convert_to_quads,poke,spin,screw,solidify,wireframe,symmetrize,vertices_smooth,flip_quad_tessellation}.rs`, `tests/golden/cases/edit/misc/**` | 2.1, 2.2 *(impl)*, 2.5 *(soft: `solidify`, `spin` extrude)*, 2.7/2.8/2.9 *(soft: `symmetrize` = bisect + weld + reverse)* | `separate` objects/active/selection as Blender; triangulate FIXED at T1, BEAUTY at T2; spin requires `axis`. |
| 2.11 | W2 | Bevel | `crates/frap-bmesh/src/ops/bevel.rs` + `ops/bevel/**`, `crates/frap-ops-mesh/src/ops/bevel.rs`, `tests/golden/cases/edit/bevel/**` | 2.1, 2.2 *(impl)* | segments = 1 at T1; N segments at T2 (tol 1e-4) incl. verified counts (all edges seg 2 → V56 E108 F54); vertex bevel; `offset=0` no-op hint. |
| 2.12 | W2 | Edit-mode transforms | `crates/frap-ops-transform/src/edit/**` (`TRANSFORM_MODE_HANDLERS` entries; the dispatchers stay with 1.21), `crates/frap-ops-transform/src/ops/{shrink_fatten,edge_slide,vert_slide,tosphere,edge_crease,edge_bevelweight,vert_crease,push_pull,mirror,vertex_random,transform}.rs`, `tests/golden/cases/edit/transform/**` | 2.1, 2.2 *(impl)* | World-space values; pivots incl. ACTIVE_ELEMENT and INDIVIDUAL_ORIGINS; `orient_type='NORMAL'`; proportional falloffs. |
| 2.13 | W3 | Workflows, transcripts, `frap.describe` | `tests/golden/cases/edit/workflows/**`, `tests/cli/m2/**`, `docs/agents.md` (reassigned), `crates/frap-ops-frap/src/ops/describe.rs`, `crates/frap-schema/src/provider.rs`, `spec/frap/ops/frap.json` (M2W0-8) | W2 *(impl)* | ≥ 20 multi-op modelling workflows (chair, mug, table) as goldens; agent guide updated with edit-mode recipes; `bpy.ops.frap.describe(target=…)` returns the `--describe` fragment through `DESCRIBE_PROVIDERS` (`DESCRIBE_TARGET` for unknown targets). |

### 22.5 M3: Materials, images, UVs, complete static glTF

**Shippable**: all glTF-exportable shader patterns and every KHR material extension, images (load/new/pack/save, embedded textures, samplers), cameras and lights in glTF, vertex colours, multiple UV sets, multi-primitive meshes, extras, all export filters and formats, UV ops; multiple scenes (`bpy.data.scenes.new`, `scene.new`/`delete`, `context.window.scene`, glTF `scenes[]`, `use_active_scene`), LINES/POINTS primitives and strip/fan import, `PointCloud` import/export. **Exit**: ≥ 60 static-glTF goldens canonically equal to Blender covering every core static feature, multi-scene files, LINES/POINTS, KHR_lights_punctual, KHR_texture_transform and every `KHR_materials_*`; the differential round-trip law (§17.2) on all of them.

| WP | Wave | Title | Owns | Needs | Acceptance |
|---|---|---|---|---|---|
| 3.0 | W0 | Contract delta (merged with 4.0, decision log M34W0) | `LoweredMaterial` extension fields, `frap-uv` `ParamMesh` API, glTF material-extension and export extension points, module stubs | M2 | Compiles; contract pins in `crates/frap-ops-all/tests/contracts.rs`. |
| 3.1 | W1 | Node trees complete, world | `crates/frap-nodes/src/availability/**` (reassigned), `crates/frap-nodes/src/builtin_groups/**` except `smooth_by_angle.rs` (4.1), node RNA for every supported node (`crates/frap-nodes/src/{rna.rs,rna/**,tree/avail.rs}`, reassigned), `crates/frap-ops-data/src/funcs/{nodes_*,node_links_*}.rs` (reassigned), `crates/frap-ops-data/src/ops/world__new.rs`, `crates/frap-ops-object/src/ops/material_slot_move.rs` (M34W0-4), `tests/golden/cases/nodes/**` | 3.0 | Socket enabled sets equal Blender per mode for every supported node; glTF Material Output group identical to the add-on's; World node tree defaults. |
| 3.2 | W1 | Images | `crates/frap-scene/src/id/image.rs` (reassigned), `crates/frap-scene/src/hooks/image_*.rs`, `crates/frap-ops-data/src/ops/image__*.rs`, `crates/frap-ops-data/src/funcs/{blend_data_images_*,image_*}.rs` (`blend_data_images_{new,remove}.rs` reassigned), `crates/frap-ops-data/src/hooks/image_*.rs`, `crates/frap-project/src/scene_map/**` (reassigned for W1, M34W0-3), `tests/golden/cases/images/**` | 3.0 | `images.new/load`, `image.new/open/pack/unpack/reload/save/save_as`, `Image.pack/save/save_render/scale/pixels`; packed images in `.frap` (original bytes). |
| 3.3 | W1 | UV core | `crates/frap-uv/**` | 3.0 | LSCM of the seamed cube within 1e-4 of Blender (solver-dependent, so a tolerance, not exactness); cube/cylinder/sphere projection equal Blender's headless output [MESH App. B.5]; smart_project B.5 layout; packer invariants (T3). |
| 3.4 | W1 | Cameras and lights in glTF | `crates/frap-io-gltf/src/{export,import}/{cameras,lights}.rs` (import reassigned; export wired by W0 into `vtree.rs`/`nodes.rs`), camera/light RNA hooks `crates/frap-scene/src/id/{camera,light}.rs` (reassigned) and `crates/frap-scene/src/hooks/{camera_*,light_*}.rs`, `crates/frap-ops-object/src/ops/{view3d__camera_to_view_selected,view3d__object_as_camera}.rs`, `tests/golden/cases/gltf/lights_cameras/**` | 3.0 | W→cd/lux conversion, spot cones, AREA warning, `range` rules, sensor fit/aspect; `light.type` re-typing. |
| 3.5 | W2 | Material lowering: core features | `crates/frap-material/src/patterns/{normal_map,occlusion,emission_strength,alpha,texture_transform,vertex_color,uv_source,sampler,unlit}.rs` and the M1 `patterns/{base_color,metallic_roughness,emission,double_sided}.rs` (reassigned), the raise/lower helpers `crates/frap-material/src/lowered/{build,graph}.rs` (reassigned; the plan's `raise/**`, M34W0-3), `crates/frap-io-gltf/src/export/{materials/mod.rs,materials/core_*.rs,images.rs}` (reassigned), `crates/frap-io-gltf/src/import/{materials,images}.rs` (reassigned), `crates/frap-gltf/src/ext/khr_materials_unlit.rs`, `tests/golden/cases/gltf/materials_core/**` | 3.1, 3.2 *(soft)* | Alpha modes from node patterns; ORM packing; image passthrough vs re-encode; texture transform `extensionsRequired`; fake COLOR_0. |
| 3.6 | W2 | Material lowering: KHR_materials_* | `crates/frap-material/src/patterns/{clearcoat,sheen,transmission,volume,ior,specular,iridescence,anisotropy,dispersion,variants}.rs`, `crates/frap-io-gltf/src/export/materials/ext/**` and `crates/frap-io-gltf/src/import/materials_ext/**` (automod directories replacing the plan's `ext_*.rs` / `materials_ext_*.rs`, registering into `crates/frap-io-gltf/src/material_ext.rs`, which 3.6 owns after W0), `crates/frap-gltf/src/ext/khr_materials_*.rs` (except unlit, emissive_strength), `tests/golden/cases/gltf/materials_ext/**` | 3.1 | One Blender export golden per extension; `ior` only with transmission/volume/specular [GLTF §4.5]; import raise round-trip. |
| 3.7 | W2 | Mesh attribute and primitive-mode completeness | `crates/frap-io-gltf/src/export/mesh/writers/{color,tangent,texcoord_extra}.rs`, `crates/frap-io-gltf/src/export/mesh/extract/{loose_edges,loose_points,pointcloud}.rs`, `crates/frap-io-gltf/src/export/{extras,filters,formats}.rs` (W0 wired `extras` into nodes, meshes, materials and scenes) and `crates/frap-io-gltf/src/export/{vtree,write}.rs` (reassigned: M1 filters and formats live there), `crates/frap-io-gltf/src/import/{mesh,extras}.rs` (reassigned), `crates/frap-scene/src/id/pointcloud.rs` (reassigned), `crates/frap-project/src/scene_map/**` (reassigned for W2), `crates/frap-geom/src/mikktspace.rs`, `crates/frap-ops-data/src/funcs/mesh_calc_tangents.rs` (OPERATORS.md §E `Mesh.calc_tangents`), `tests/golden/cases/gltf/attributes/**` | 3.0 | Vertex colours (`export_vertex_color` modes), multiple UV sets, tangents (w sign), multi-primitive meshes, custom props → extras, filters (selection/visible/renderable/collection), GLB/GLTF_SEPARATE/GLTF_EMBEDDED; LINES/POINTS export (`use_mesh_edges`/`use_mesh_vertices`) and strip/fan/line/point import equal to Blender; `import_point_as_pointcloud` creates a POINTCLOUD object. No edit of `mesh/primitives.rs` (extension points only). |
| 3.8 | W2 | UV operators | `crates/frap-ops-uv/**`, `tests/golden/cases/uv/**` | 3.3 *(impl)* | `uv.unwrap` CONFORMAL (ANGLE_BASED via LSCM fallback until M8, flagged), `smart_project`, projections, `reset`, `pack_islands`, `average_islands_scale`, `seams_from_islands`, `mark_seam`, `follow_active_quads`, `lightmap_pack`, `pin`; `project_from_view` uses `scene.camera` (frap-extended). |
| 3.10 | W1 | Multiple scenes | `crates/frap-scene/src/{window.rs,scenes.rs}` (created by W0), `crates/frap-scene/src/id/scene.rs` and `crates/frap-scene/src/hooks/window_*.rs` (reassigned), `crates/frap-ops-data/src/funcs/blend_data_scenes_*.rs`, `crates/frap-ops-object/src/ops/{scene__new,scene__delete,hide_view_set,hide_view_clear,make_single_user,make_links_data}.rs` (M34W0-4), `crates/frap-io-gltf/src/export/scenes.rs` (created by W0 with the M1 single-scene gathering) and `crates/frap-io-gltf/src/import/scenes.rs` (reassigned), `tests/golden/cases/gltf/scenes/**`, `tests/golden/cases/scenes/**` | 3.0 | `bpy.data.scenes.new/remove`, `scene.new(type=…)`, `scene.delete`; `bpy.context.window.scene = …` switches `context.scene`/`view_layer`; multi-scene export (default and `use_active_scene=True`) and import goldens vs Blender; scenes round-trip in `.frap`. |
| 3.9 | W3 | M3 suite | `tests/golden/cases/gltf/m3_workflows/**`, `tests/cli/m3/**` | W2 *(impl)* | Exit criteria above. |

### 22.6 M4: Modifier stack

**Shippable**: non-destructive modelling with SUBSURF, MIRROR, ARRAY, BEVEL, SOLIDIFY, BOOLEAN, TRIANGULATE, DECIMATE, WELD, DISPLACE, EDGE_SPLIT, WEIGHTED_NORMAL, SMOOTH, CAST, SIMPLE_DEFORM, WIREFRAME, SCREW, MASK, BUILD, NODES ("Smooth by Angle"); modifier operators; `export_apply`; renders show modifiers. **Exit**: evaluated-mesh goldens per modifier (defaults + key settings) at T2 (Boolean EXACT T3).

| WP | Wave | Title | Owns | Needs | Acceptance |
|---|---|---|---|---|---|
| 4.0 | W0 | Contract delta (merged with 3.0, decision log M34W0) | `ModifierTypeInfo`, `ModEvalCx`, `#[modifier]`, `Subdivider`, `BooleanSolver` stubs, the stack walk and `frap-eval` wiring, module stubs | M2 | Compiles. (If M3 runs in parallel, one integrator WP does 3.0 and 4.0 together.) |
| 4.1 | W1 | Stack infra and modifier operators | `crates/frap-modifiers/src/{lib,stack,registry}.rs`, `crates/frap-modifiers/src/types/nodes.rs`, `crates/frap-eval/src/geometry.rs` (created by W0) and `crates/frap-eval/src/cache/{mesh,fingerprint}.rs` (reassigned: `depends_on` in the fingerprint), `crates/frap-scene/src/id/modifier.rs` and `crates/frap-scene/src/hooks/{modifier_*,object_modifiers_*}.rs` (reassigned), `crates/frap-nodes/src/builtin_groups/smooth_by_angle.rs`, `crates/frap-ops-object/src/ops/{modifier_*,modifiers_clear,shade_auto_smooth,subdivision_set,convert}.rs`, `crates/frap-ops-data/src/funcs/object_modifiers_*.rs`, `tests/golden/cases/modifiers/stack/**` | 4.0 | `modifier_apply` reports and multi-user error; `show_*` flags; pin-to-last; `evaluated_get`/`to_mesh`; `shade_auto_smooth` creates the linked group and two `Library` entries as Blender does; NODES dispatches on (library basename, name) and ignores a same-named local group; Smooth by Angle equals Blender; unknown types pass through with WARNING. |
| 4.2 | W1 | Simple modifiers A | `crates/frap-modifiers/src/types/{mirror,array,solidify,weld,displace,edge_split}.rs`, `crates/frap-scene/src/id/texture.rs` (reassigned; field set frozen by W0) and `crates/frap-scene/src/hooks/texture_*.rs`, `crates/frap-ops-data/src/funcs/blend_data_textures_*.rs`, `tests/golden/cases/modifiers/a/**` | 4.0, 4.1 *(soft: `modifiers.new` and `object.modifier_add` for goldens, M34W0-2)* | Goldens per type incl. mirror bisect/clipping, array offsets and merge, solidify simple/complex rims. |
| 4.3 | W1 | Simple modifiers B | `crates/frap-modifiers/src/types/{triangulate,smooth,cast,simple_deform,wireframe,screw,weighted_normal,mask,build,normal_edit,laplaciansmooth}.rs`, `tests/golden/cases/modifiers/b/**` | 4.0, 4.1 *(soft, M34W0-2)* | Goldens per type. |
| 4.4 | W1 | Subdivision | `crates/frap-subdiv/**`, `crates/frap-modifiers/src/types/subsurf.rs`, `tests/golden/cases/modifiers/subsurf/**` | 4.0, 4.1 *(soft, M34W0-2)* | Levels 1–3, creases, `uv_smooth`, `boundary_smooth` at T2; Blender layout ordering pass towards T1; both backends behind the trait (feature gate). |
| 4.5 | W1 | Bevel and decimate modifiers | `crates/frap-modifiers/src/types/{bevel,decimate}.rs`, `crates/frap-bmesh/src/ops/{decimate_collapse,decimate_unsubdivide,decimate_dissolve}.rs` (W0 ported the two wrappers; collapse is a stub), `crates/frap-ops-mesh/src/ops/decimate.rs`, `tests/golden/cases/modifiers/bevel_decimate/**` | 4.0, M2 bevel, 4.1 *(soft, M34W0-2)* | Limit methods (ANGLE default, WEIGHT via `bevel_weight_edge`); decimate modes within tolerance. |
| 4.6 | W1 | Boolean | `crates/frap-boolean/**`, `crates/frap-modifiers/src/types/boolean.rs`, `crates/frap-ops-mesh/src/ops/{intersect,intersect_boolean}.rs`, `tests/golden/cases/modifiers/boolean/**`; the M2 Auto Merge with split leftover (M34W0-11): `crates/frap-bmesh/src/ops/intersect_edges.rs`, `crates/frap-ops-transform/src/edit/automerge.rs` (wired by W0), `crates/frap-ops-mesh/src/ops/extrude_manifold.rs` (reassigned), `tests/golden/cases/edit/automerge_split/**` | 4.0, 4.1 *(soft, M34W0-2)* | MANIFOLD at T2, EXACT at T3; UV/material transfer; FLOAT aliases EXACT with WARNING until M8. |
| 4.7 | W2 | Integration | `crates/frap-io-gltf/src/export/apply.rs` and `crates/frap-io-gltf/src/export/mesh/gather.rs` (reassigned: per-object evaluated meshes and their sharing), `crates/frap-render-scene/src/build/evaluated.rs`, `tests/golden/cases/modifiers/integration/**`, `tests/cli/m4/**` | W1 *(impl)* | `export_apply=True` goldens; renders show modifiers; linked-duplicate mesh sharing rules in export. |

**Merged plan (M3 ‖ M4, decision log M34W0).** M3 and M4 run together after one integrator Wave 0 (3.0 + 4.0): **W1** = 3.1, 3.2, 3.3, 3.4, 3.10, 4.1, 4.2, 4.3, 4.4, 4.5, 4.6 (11 WPs); **W2** = 3.5, 3.6, 3.7, 3.8, 4.7; **W3** = 3.9. No two WPs of one wave own the same path (M34W0-3). **Golden prerequisite set (M3)**: `bpy.data.images.new/load` and `image.pack` (3.2), `bpy.data.scenes.new` and `context.window.scene` (3.10), `nodes.new`/`links.new` (M1), `object.camera_add`/`light_add` (M1), all W1, used by W2 goldens. **Golden prerequisite set (M4)**: `obj.modifiers.new` and `object.modifier_add` (4.1, W1, the same wave as 4.2–4.6, which therefore list 4.1 as *(soft)*: their evaluators are unit-tested against Blender dumps first, and their goldens run once 4.1 lands; W0 already routes evaluation through the registered evaluators, so nothing else is missing). Platform rules for every new GPU test: they run on Metal (macOS), lavapipe (Linux) and WARP (Windows) in `ci.yml`'s test job and `gpu.yml`, and skip with a visible notice where `FRAP_GPU_OPTIONAL=1` finds no adapter (§20.5).

### 22.7 M5: EEVEE-like renderer and colour management

**Shippable**: `BLENDER_EEVEE` with PBR on `LoweredMaterial`, shadows, HDRI/IBL, transparency, TAA; every view transform and look; all output formats. **Exit**: EEVEE goldens SSIM ≥ 0.90 vs Blender on 12 scenes, strict self-goldens; colour goldens ΔE < 1.

| WP | Wave | Title | Owns | Needs | Acceptance |
|---|---|---|---|---|---|
| 5.0 | W0 | Contract delta (merged with 6.0, decision log M56W0) | `frap-render/src/eevee/mod.rs` (`Eevee`, `EEVEE_FEATURES`, the engine gate `pipeline::IMPLEMENTED`), the EEVEE module stubs, `RenderService::supports_engine`, `REeveeSettings`/`RLightEevee`/`RenderScene::image_ids`/`RMesh::{uv_names,uv_active_render,color_names}`, `frap-color` view-transform modules, `frap-imbuf/src/encode/` | M3, M4 | Compiles. (If M6 runs in parallel, one integrator WP does 5.0 and 6.0 together.) |
| 5.1 | W1 | EEVEE core | `crates/frap-render/src/eevee/{pipeline,lights,shadows,taa,film}.rs` (sets `pipeline::IMPLEMENTED`, which routes `BLENDER_EEVEE` and `CYCLES` to the engine), `crates/frap-render/src/shaders/eevee_*.wgsl`, `tests/golden/cases/render_eevee/core/**` | 5.0 | Light-unit golden on a diffuse plane (±5% luminance) per light type; shadow placement tests; one EEVEE uber-shader with feature uniforms; `Rgba32Float` TAA accumulation; cold-start render time on WARP and lavapipe within the CI budget. |
| 5.2 | W1 | World and IBL | `crates/frap-render/src/eevee/ibl/**` (`world_lighting`, the signature 5.1 calls for `RWorld::Hdri`; 5.1 lights `RWorld::Color` itself), `assets/brdf_lut.bin`, `crates/frap-render-scene/src/build/world.rs`, `tests/golden/cases/render_eevee/world/**` | 5.0 | HDRI scenes SSIM ≥ 0.90 vs Blender. |
| 5.3 | W1 | Colour management complete | `crates/frap-color/src/{agx,filmic,pbr_neutral,aces,false_color,looks}.rs` (each module's `BAKED` list feeds `lut::baked`), `crates/frap-color/src/display.rs` and `crates/frap-color/tests/**` (reassigned: `ViewTransform`/`look_applies` for every transform and look), `assets/luts/**`, `tools/blender/bake_view_luts.py` (reassigned) | 5.0 | Every view transform × look within ΔE < 1 on CPU, measured on held-out colours only (§19.5). |
| 5.4 | W1 | Output formats | `crates/frap-ops-render/src/output.rs` and `crates/frap-ops-render/src/output/**` (reassigned), `crates/frap-imbuf/src/encode/**` (W0 moved the M1 encoders there), `tests/golden/cases/render/output/**` | 5.0 | PNG 8/16, JPEG, WEBP, OPEN_EXR, HDR, TARGA, BMP; `color_mode` BW/RGB/RGBA; `film_transparent`. |
| 5.5 | W2 | Shading features | `crates/frap-render/src/eevee/features/**` (an `automod` directory: one file per feature registering an `EeveeFeature`), and the M5 leftovers of the renderer (decision log M56W0-4): `crates/frap-render/src/workbench/**` (reassigned: Workbench `VERTEX`/`TEXTURE` colour types), `crates/frap-render-scene/src/build/{images,mesh}.rs` (reassigned: `Image::acquire`, layer names), `crates/frap-ops-render/src/ops/frap__render_contact_sheet.rs` (reassigned: `color_type_left_out`), `tests/golden/cases/render_eevee/features/**` | 5.1 *(impl)* | Normal maps, alpha blend/dither, transmission, clearcoat, sheen, emission, vertex colours: per-feature self-goldens + loose Blender goldens. |
| 5.6 | W2 | Render goldens | `tests/golden/cases/render_eevee/scenes/**`, `tests/cli/m5/**` | 5.1, 5.2 *(impl)* | Exit criteria. |

### 22.8 M6: Animation, rigging, morph targets

**Shippable**: keyframes on any animatable property, slotted actions, F-curves, interpolation, NLA; armatures (edit/pose), vertex groups and weights, ARMATURE modifier and parenting; shape keys; glTF animation (all `export_animation_mode`s), skins, morph targets, KHR_animation_pointer; import of all of them; `render.render(animation=True)`. **Exit**: animation goldens on sampled values (±1e-5) incl. the time-boundary sequence of §7.3, glTF animation/skin/morph goldens for every mode, differential round-trip law.

| WP | Wave | Title | Owns | Needs | Acceptance |
|---|---|---|---|---|---|
| 6.0 | W0 | Contract delta (merged with 5.0, decision log M56W0) | the `frap-anim` data model and evaluation signatures, `Armature`/`Bone`/`Pose`/`PoseBone`/`Key`/`ShapeKey`/`EditArmature`/`EditBone`, `frap-eval` `pose.rs`/`key_eval.rs` wired into evaluation, `frap-ops-anim` crate skeleton, glTF animation/skin/morph/pointer stubs and `NodeMap`, the golden `anim:` dump | M3, M4 | Compiles. (Animation, skins and morphs do not need EEVEE; `render.render(animation=True)` needs only M1's render op. M5 and M6 can run in parallel.) |
| 6.1 | W1 | Animation data and evaluation | `crates/frap-anim/**` (data model frozen by W0, RNA bindings in `rna.rs`), the Action data API: `crates/frap-ops-anim/src/funcs/{action_*,action_slots_*,action_layers_*,action_strips_*,action_channelbag*,channelbag_*,fcurve_*,fcurve_keyframe_points_*,anim_data_*,id_animation_data_*}.rs`, `crates/frap-ops-data/src/funcs/blend_data_actions_*.rs`, `crates/frap-scene/src/hooks/{object_animation_data*,id_animation_data*,anim_data_*,action_*}.rs`, `tests/golden/cases/anim_eval/**` | 6.0 | F-curve sampling equals Blender for every interpolation, easing, handle type, extrapolation; NLA blending. |
| 6.2 | W1 | Armatures and pose | `crates/frap-scene/src/id/armature*.rs`, `crates/frap-scene/src/edit_armature.rs`, `crates/frap-eval/src/pose.rs`, `crates/frap-ops-object/src/ops/{armature_add,posemode_toggle}.rs`, `crates/frap-ops-object/src/modes/{edit_armature,pose}.rs`, `crates/frap-ops-anim/src/ops/{armature__*,pose__*}.rs`, `crates/frap-ops-transform/src/pose/**` (`TRANSFORM_MODE_HANDLERS` entries), `crates/frap-ops-data/src/funcs/{blend_data_armatures_*,armature_edit_bones_*,armature_bones_*}.rs`, `crates/frap-scene/src/hooks/{armature_*,bone_*,edit_bone_*,pose_*,object_pose*}.rs`, `crates/frap-scene/src/context/rna.rs` (reassigned: the bone members `active_bone`, `active_pose_bone`, `selected_bones`, `selected_editable_bones`, `editable_bones`, `visible_bones`, `selected_pose_bones*` over W0's `Context` accessors, M56W0-12), `tests/golden/cases/armature/**` except `armature/deform/**` | 6.0 | Edit/pose/rest bones; `mode_set` EDIT for armatures and POSE through `MODE_HANDLERS` (no edit of `mode_set.rs`); bone transforms in pose mode through the transform dispatchers. |
| 6.3 | W1 | Vertex groups and shape keys | `crates/frap-scene/src/id/key.rs`, `crates/frap-ops-data/src/funcs/{object_vertex_groups_*,vertex_groups_*,vertex_group_*,object_shape_key_*,shape_key_*,mesh_transform}.rs` (the plan's stems corrected, M56W0-3), `crates/frap-scene/src/hooks/{object_vertex_groups*,vertex_group_*,object_shape_key*,shape_key_*,key_*,mesh_shape_keys*}.rs`, `crates/frap-ops-object/src/ops/{vertex_group_*,vertex_weight_*,shape_key_*,join_shapes,modifier_apply_as_shapekey}.rs`, `crates/frap-ops-object/src/ops/join.rs` and `ops/join/**` (reassigned: shape keys), `crates/frap-ops-mesh/src/ops/{blend_from_shape,shape_propagate_to_all,separate}.rs` (`separate` reassigned), `crates/frap-ops-object/src/modes/edit_mesh.rs` (reassigned: shape layers on entering/leaving Edit Mode), `crates/frap-bmesh/src/shape.rs`, `crates/frap-eval/src/key_eval.rs`, `crates/frap-eval/src/cache/fingerprint.rs` (reassigned: the `Key`'s generation), `tests/golden/cases/weights_shapes/**` | 6.0 | `VertexGroup.add` modes and `weight()` errors; `shape_key_add(from_mix)`; edit-mode basis propagation; relative mixing. |
| 6.4 | W2 | Keying, frames, NLA | `crates/frap-ops-anim/src/{keying.rs,funcs/**,ops/{anim__*,keyframe_*,screen__*,nla__*}.rs}` (the funcs 6.1 created stay 6.1's), `crates/frap-eval/src/animate.rs`, `crates/frap-scene/src/context/rna.rs` (reassigned after 6.2: `active_action`, `active_editable_fcurve`, `active_nla_track`, `active_nla_strip`, M56W0-12), `crates/frap-project/src/scene_map/**` (reassigned: `.frap` persistence of actions, NLA, armatures and poses, shape keys and vertex groups, M56W0-4), `tests/golden/cases/anim/**` | 6.1 *(impl)* | `CubeAction`/`OBCube`/"Object Transforms"; `KE` slot in the object's action; `NT` actions on the embedded node tree (`Root::Embedded`); `frame_set` writes originals; `frame_current` applies animation only at evaluation boundaries (golden: `frame_current = 10; location.x = 1; keyframe_insert("location")` equals Blender); render/export clones leave manual edits of animated channels intact (deviation test); `screen.frame_*`; `.frap` round trip of the M6 data. |
| 6.5 | W2 | Armature deform and parenting | `crates/frap-modifiers/src/types/armature.rs`, `crates/frap-ops-object/src/ops/parent_set.rs` (reassigned), `crates/frap-ops-object/src/support/deform.rs` (reassigned: `deformed_by` with the virtual armature-parent and shape-key modifiers), `tests/golden/cases/armature/deform/**` | 6.2, 6.3 *(impl)* | LBS equals Blender; `ARMATURE_NAME`/`ARMATURE_ENVELOPE`; `ARMATURE_AUTO` in M8. |
| 6.6 | W2 | Animation render | `crates/frap-ops-render/src/ops/render__render.rs` (reassigned) | 6.4 *(soft)* | Frame paths, `frame_step`, is-set overrides; one State clone per frame; `frame_current` after the call equals Blender's. |
| 6.7 | W3 | glTF animation export/import | `crates/frap-io-gltf/src/{export,import}/animation/**` (export: `animation::gather` after the scenes, targets through `export::node_map::NodeMap`; import: `animation::IMPLEMENTED` replaces the WARNING, bones through `VNode::blender_bone_name`), the `export_animation*` entries of `SUPPORTED_EXPORT_PROPS` | 6.4 *(impl)*, 6.8 *(soft: bone targets)* | Every `export_animation_mode`; sampling vs CUBICSPLINE vs STEP; `fps_base` quirk; import with NLA stashing. |
| 6.8 | W3 | glTF skins and morphs | `crates/frap-io-gltf/src/{export,import}/skins.rs`, `crates/frap-io-gltf/src/export/mesh/writers/skin.rs`, `crates/frap-io-gltf/src/export/mesh/post/morph_targets.rs`, `crates/frap-io-gltf/src/import/morph.rs`, `crates/frap-io-gltf/src/export/{nodes,vtree}.rs` and `crates/frap-io-gltf/src/import/{nodes,vnode,mesh}.rs` (reassigned: bones as nodes, `NodeMap::bones`, `VNode::blender_bone_name`, shape keys from targets) | 6.5, 6.3 *(impl)* | IBMs, joints, deform-bone filter, `bone_heuristic`, `guess_original_bind_pose`; morph targets incl. sparse heuristic and weights animation. |
| 6.9 | W3 | KHR_animation_pointer | `crates/frap-io-gltf/src/{export,import}/pointer/**` (called by 6.7's exporter and importer), `crates/frap-gltf/src/ext/khr_animation_pointer.rs` | 6.4 *(impl)*, 6.7 *(soft)* | Material (embedded node tree), light and camera property animation round-trips. |
| 6.10 | W2 | Vertex-group options of the M4 modifiers | `crates/frap-modifiers/src/types/{mask,displace,smooth,laplaciansmooth,cast,simple_deform,solidify,wireframe,weld,decimate,normal_edit,weighted_normal}.rs` (reassigned: `vertex_group`/`invert_vertex_group` and the per-type group settings; BEVEL's `VGROUP` limit already reads `dvert`), `tests/golden/cases/modifiers/vgroups/**` | 6.3 *(impl)* | Every listed option equals Blender (MASK `VERTEX_GROUP`; `ARMATURE` mode with 6.5 *(soft)*); the deviations §2 vertex-group row closed. |

**Merged plan (M5 ‖ M6, decision log M56W0).** M5 and M6 run together after one integrator Wave 0 (5.0 + 6.0): **W1** = 5.1, 5.2, 5.3, 5.4, 6.1, 6.2, 6.3 (7 WPs); **W2** = 5.5, 5.6, 6.4, 6.5, 6.6, 6.10; **W3** = 6.7, 6.8, 6.9. The only change from the plan the milestone tables imply is the new WP 6.10 (W2), which takes the vertex-group options of the M4 modifiers out of 6.5 (they were listed for M6 in deviations §2 with no owner). No two WPs of one wave own the same path (M56W0-3). **Golden prerequisite set (M6)**: the Action data API (`bpy.data.actions.new`, `slots.new`, `layers.new`, `strips.new`, `channelbag(…, ensure=True)`, `fcurves.new`, `keyframe_points.insert/add`, `animation_data_create`; 6.1, W1) for 6.4's and 6.6's goldens; `object.armature_add` and `mode_set` EDIT/POSE (6.2, W1) and `vertex_groups.new`/`VertexGroup.add`/`shape_key_add` (6.3, W1) for 6.5's and 6.10's; `keyframe_insert` (6.4, W2) for the W3 glTF goldens. 6.1's own goldens build their curves with its data API; 6.2's and 6.3's use their own operators. **Golden prerequisite set (M5)**: `object.light_add`, `camera_add`, `world` node edits and `images.load` (M1/M3, done); `render.render(write_still=True)` (M1). Platform rules for every new GPU test (5.1, 5.2, 5.5, 5.6): Metal, lavapipe and WARP in `ci.yml` and `gpu.yml`, a visible skip under `FRAP_GPU_OPTIONAL=1`, no reliance on driver folding of pack/unpack round trips (`workbench.wgsl`'s integer-mask `rtz16`); file errors through `frap_rna::error::strerror` with native paths, separator-agnostic goldens and transcripts.

### 22.9 M7: Compression and spec-complete glTF

**Shippable**: Draco, meshopt (EXT and KHR), KHR_mesh_quantization, EXT_texture_webp (lossy), KHR_texture_basisu (read; encode behind feature), EXT_mesh_gpu_instancing (`export_gpu_instances`), KHR_materials_variants complete, KHR_node_visibility and diffuse_transmission behind `frap_*`, `frap_compat='SPEC'`. **Exit**: Blender imports every frap-compressed file; frap decodes every Blender-compressed file within quantization bounds.

| WP | Wave | Title | Owns | Needs | Acceptance |
|---|---|---|---|---|---|
| 7.0 | W0 | Contract delta | codec trait stubs | M3, M6 | Compiles. |
| 7.1 | W1 | meshopt and quantization | `crates/frap-codec-mesh/src/meshopt.rs`, `crates/frap-io-gltf/src/export/mesh/post/{meshopt,quantize}.rs`, `crates/frap-io-gltf/src/export/gltfpack.rs`, `crates/frap-gltf/src/ext/{ext_meshopt_compression,khr_meshopt_compression,khr_mesh_quantization}.rs` | 7.0 | Both meshopt variants decode in Blender; gltfpack prop mapping; `frap_mesh_quantization`. |
| 7.2 | W1 | Draco | `crates/frap-codec-mesh/src/draco*.rs`, optional `crates/frap-draco-sys/`, `crates/frap-gltf/src/ext/khr_draco_mesh_compression.rs`, `crates/frap-io-gltf/src/{export/mesh/post/draco.rs,import/decompress.rs}` | 7.0 | Spike gate (draco-core): Blender imports frap Draco, frap decodes Blender Draco, within quantization bounds; else enable the cc-built fallback. |
| 7.3 | W1 | Texture formats | `crates/frap-imbuf/src/{ktx2,basis,webp_lossy}.rs`, `crates/frap-io-gltf/src/export/images_{webp,ktx2}.rs`, `crates/frap-gltf/src/ext/{ext_texture_webp,khr_texture_basisu}.rs` | 7.0 | WebP with fallback image; KTX2 import + render; `frap_texture_format='KTX2'` with `basisu-encode`. |
| 7.4 | W1 | Spec-complete writer | `crates/frap-io-gltf/src/export/spec_complete/**`, `crates/frap-gltf/src/ext/{khr_node_visibility,khr_materials_diffuse_transmission}.rs` | 7.0 | `frap_compat='SPEC'`, `frap_interleave`, UNSIGNED_BYTE indices, node visibility; output passes the Khronos validator. |
| 7.5 | W1 | Instancing and variants | `crates/frap-io-gltf/src/{export,import}/{instancing,variants}.rs`, `crates/frap-scene/src/id/scene_gltf_variants.rs`, `crates/frap-gltf/src/ext/{ext_mesh_gpu_instancing,khr_materials_variants}.rs` | 7.0 | Variants with the add-on's data model, created through the RNA that exists while `KHR_materials_variants_ui` is on (§4.1), import turns the preference on; `export_gpu_instances` with the add-on's rule (children of an Empty) and instancing import, goldens vs Blender. |

### 22.10 M8: Parity hardening (continuous, after M2)

One WP per row, parallel: 8.1 ABF++ and SLIM in `frap-uv`; 8.2 bevel multi-segment corner exactness (T2 → T1); 8.3 remaining P1/P2 operators, one WP per namespace (see OPERATORS.md); 8.4 `ed.undo`/`ed.redo` via the snapshot ring; 8.5 `ARMATURE_AUTO` heat weights; 8.6 Boolean FLOAT solver; 8.7 differential fuzzer and `cargo fuzz` targets; 8.8 performance budgets (§21.7); 8.9 Blender 5.2.x spec-refresh routine; 8.10 `frap-lua` spike against the Session API; 8.11 bit-exact OpenSubdiv face-varying evaluation (D-124: the glTF vertex split of subdivided UV meshes).

### 22.11 Critical path

```
M0 ─► 1.0 ─► 1.1/1.2 ─► 1.10/1.12/1.14 ─► 1.19/1.22 ─► W4 of M1 ─► 2.1/2.2 ─► 2.5…2.12 ─► (M3 ‖ M4) ─┬─► M5
                                                                                                   └─► M6 ─► M7
              1.5 ─► 1.16/1.17 ─► 1.23 ─► 3.5/3.6/3.7 ─► 6.7/6.8 ─► 7.x
              1.7 ─► 1.24 ─► 2.3 ─► 5.1
```

M3 and M4 both need only M2 and can overlap when staffing allows (their W0s are then merged into one integrator WP). M5 needs M3 and M4; M6 needs M3 and M4 (not M5); M5 and M6 can overlap the same way. M7 needs M6. Steady state: 6–10 WPs in flight.

---

## 23. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Licence D1 resolved as permissive | Many ops capped at T2/T3; no Suzanne table | Clean-room implementations behind the same traits; tier table shows the gap honestly. |
| Bevel (`bmesh_bevel.cc`, ~8k lines) | M2 slip | Dedicated WP 2.11; seg = 1 first; T2 for N segments at 1e-4; exactness deferred to M8. |
| UV unwrap/pack parity | Unequal layouts | Parametrizer port per island; packing judged by T3 invariants permanently. |
| Young crates (`manifold-rust`, `opensubdiv-rs`, `draco-core`) | Correctness, abandonment | Exact pins behind frap traits; own Catmull-Clark; cc-built Draco fallback; vendoring allowed. |
| EEVEE look parity | Low SSIM | Shared `LoweredMaterial`; LUT-exact colour management; light-unit goldens first; loose Blender + strict self-goldens. |
| wgpu majors every 3–6 months | Churn | wgpu confined to `frap-render`; `RenderScene` has no wgpu types; upgrade at most twice a year. |
| `linkme` entries dropped by the linker | Missing ops at runtime | `frap-ops-all::link()` + registry test against the source tree, in debug and release/LTO (§10.5). |
| Contracts wrong at freeze time | RFC storm during M1 | M0 walking skeleton and contract probes exercise every frozen trait before M1 W0 (§22.2). |
| Spec crates slow every build | Agent iteration time | Split `frap-spec-*` crates, typed structs only for current-milestone ops, catalog blob, CI build-time budgets (§4.2). |
| Windows shader compile time (FXC) | Slow first render, CI timeouts | Vulkan first on Windows; uber-shaders; cold-start time measured per adapter in CI. |
| MCP specification churn | Broken agent integration | Minimal hand-written server isolated in `frap-cli/src/mcp/`; `serve --jsonl` as the stable fallback. |
| Contract churn during parallel work | Merge storms | Wave 0 integrator; RFC process; additive-only changes in a milestone; `#[non_exhaustive]`. |
| Fresh-evaluation deviation surprises a golden | False failures | Blender-side normalisation in `golden_runner.py`; entries in `docs/deviations.md`. |
| Captured Blender assets (LUTs, studio lights, matcaps) | Licensing | Provenance per asset; frap-made replacements if unclear. |
| Blender 5.3 API churn | Spec drift | `spec/blender-<ver>/`, `blender_api` in `.frap`, `xtask spec-refresh --check`, migrations. |
| M1 breadth | Long first milestone | M0 first; four waves of ≤ 10 WPs with contract stubs; only "basic" subsets of each subsystem in M1; Blender-parity render gate deferred to M2. |

---

## 24. Decisions pending user sign-off

> **Resolved 2026-11-16 by the user:** D1 = (a) **GPL-3.0-or-later**; D2 follows (port Blender's Suzanne table). Porting Blender GPL source is allowed; keep provenance comments (`// Ported from blender/source/...`).

| # | Decision | Options | Recommendation | Effect on the plan |
|---|---|---|---|---|
| D1 | frap licence | (a) **GPL-3.0-or-later**: may port Blender's GPL C/C++ (polyfill/beautify, `bmo_subdivide`, `bmesh_bevel`, decimate, UV parametrizer and packer, subsurf layout, Suzanne's table). (b) MIT/Apache-2.0: clean-room from behaviour, papers and goldens. | **(a)**. Fastest, lowest-risk route to parity. All adopted dependencies (MIT, Apache-2.0, Zlib, BSD, MPL-2.0) are GPL-3 compatible; `opensubdiv-rs`'s TOST licence must be checked (the own Catmull-Clark covers it). | With (b): add ~30% to the algorithm WPs of M2–M4 and M8, accept tolerance goldens where ports would match exactly, and `mesh.primitive_monkey_add` ships a frap-designed monkey or `NotImplementedError`. |
| D2 | `primitive_monkey_add` | follows D1 | follows D1 | WP 1.4 / 1.19 |

Decisions taken by this document (reversible by RFC): factory startup as initial state with `--empty`; shortest-f32 output; always-fresh derived values and edit-mode reads (through a cached view); Blender's time semantics (animation at evaluation boundaries) with render/export on a State clone; `//` resolved in `export_scene.gltf` paths; one-argument `foreach_get`; `serve`/MCP requests default to `on_error="skip_rest"`; rejection of edit-mode mesh-data writes; rejection of non-context positional operator args; `CYCLES` rendered by the EEVEE-like engine with a WARNING; virtual 3D View with the factory Layout matrix; canonical golden comparison by default with per-op tiers.

---

## Appendix A. Facts verified for this document

Run on Blender 5.2.4 LTS (`cd5dc428cce1`), `-b --factory-startup`, on 2026-11-16, in addition to the probes recorded in [FID App. A] and the research documents.

| Probe | Result | Used in |
|---|---|---|
| `bpy.ops.mesh.primitive_cube_add.get_rna_type()` | props, types, subtypes, units, ranges, flags exactly as in §10.4; `bl_options = {'REGISTER','UNDO'}`; description "Construct a cube mesh that consists of six square faces" | §10.4 |
| `Object.bl_rna.properties['location']` | FLOAT[3], TRANSLATION, LENGTH, range ±3.4028235e38, animatable | §9.6 |
| `primitive_cube_add(sizee=2)` | `TypeError: Converting py args to operator properties:: keyword "sizee" unrecognized` | §10.2, §13 |
| `primitive_cube_add(size="x")` | `TypeError: Converting py args to operator properties: MESH_OT_primitive_cube_add.size expected a float type, not str` | §10.2 |
| `primitive_cube_add(align='FOO')` | `TypeError: Converting py args to operator properties: enum "FOO" not found in ('WORLD', 'VIEW', 'CURSOR')` | §10.2 |
| `primitive_cube_add(2)` | **returns `{'FINISHED'}`**: positional argument silently ignored | §1 #21, §10.2 |
| `bpy.ops.mesh.foo()` | `AttributeError: Calling operator "bpy.ops.mesh.foo" error, could not be found` | §10.2 |
| `bpy.ops.mesh.bevel(offset=0.1)` in object mode | `RuntimeError: Operator bpy.ops.mesh.bevel.poll() failed, context is incorrect` | §8 |
| `bpy.data.objects['Nope']` | `KeyError: 'bpy_prop_collection[key]: key "Nope" not found'` | §9.3 |
| `objects['Cube'].type = 'EMPTY'` | `AttributeError: bpy_struct: attribute "type" from "Object" is read-only` | §13.4 |
| `objects['Cube'].location = (1, 2)` | `ValueError: bpy_struct: item.attr = val: sequences of dimension 0 should contain 3 items, not 2` | §9.6 |
| `bpy.context.object = None` | `AttributeError: bpy_struct: Context property "object" is read-only` | §8 |
| `socket.path_from_id("default_value")` on Principled Base Color | `nodes["Principled BSDF"].inputs[0].default_value` | §9.3 |
| `primitive_cube_add(size=2, location=(0,0,1))` on factory | `Cube.001` / mesh `Cube.001`, selected `['Cube.001']`, 8 verts; `bpy.data.objects` = `['Camera','Cube','Cube.001','Light']` | §10.4 |
| `scene.display.shading` defaults | `type='SOLID'`, `light='STUDIO'`, `color_type='MATERIAL'` | §19.3 |
| `hasattr(Mesh, 'vertex_group_names')` | False: names are DNA-level on Mesh, exposed through `Object.vertex_groups` | §5.6 |
| Delete factory Cube (`objects.remove`), then `primitive_cube_add()` | object `Cube`, mesh `Cube.001` | §5.4, §10.3 |
| `bpy.ops.mesh.bevel.poll()` in object mode; `.idname()`, `.idname_py()` | `False`; `MESH_OT_bevel`, `mesh.bevel` | §8, §11.2 |
| `material.node_tree.is_embedded_data`, `repr(node_tree)`, `repr(inputs[0])`; `scene.collection.is_embedded_data` | `True`; `bpy.data.materials['X'].node_tree`; `bpy.data.materials['X'].node_tree.nodes["Principled BSDF"].inputs[0]`; `True` | §5.1, §9.3 |
| `object.shade_auto_smooth()` | 2 `bpy.data.libraries` (both `…/geometry_nodes_essentials.blend`); group `Smooth by Angle` with library `geometry_nodes_essentials.blend.001`; a local `node_groups.new("Smooth by Angle")` coexists and `node_groups['Smooth by Angle']` returns the local one; `ID` has `library`, `is_linked_packed`, `is_embedded_data` | §5.1, §5.4, §7.2 |
| Tuple key `node_groups['Smooth by Angle', <lib filepath or None>]` | `KeyError: 'bpy_prop_collection[key, lib]: not found'`: the group belongs to the second library with that filepath (`….blend.001`), and the lookup uses the first (pinned in WP 1.10, W2-3) | §5.2 |
| Keyed `location` z 0→10 over frames 1→11; `scene.frame_current = 6`; read; `view_layer.update()`; read | `(0,0,10)` then `(0,0,5)` | §7.3 |
| `bpy.context.window` in `-b --factory-startup`; `window.scene = bpy.data.scenes.new('S2')` | a `Window`; `context.scene` becomes `S2` | §8 |
| `export_scene.gltf` props `export_gpu_instances`, `use_active_scene`, `use_mesh_edges`; `import_scene.gltf` prop `import_point_as_pointcloud` | all present | §17.2 |
| `preferences.addons['io_scene_gltf2'].preferences.KHR_materials_variants_ui`; `hasattr(Scene, 'gltf2_KHR_materials_variants_variants')` | `False`; `False` (factory startup) | §4.1 |
| Operator counts per namespace | object 250, mesh 167, uv 55, transform 27, armature 49, pose 51, anim 64, nla 39, render 16, wm 117, image 49, scene 39, collection 12, geometry 9 | OPERATORS.md |

---

## Decision log (review of v1.0)

An adversarial review of v1.0 raised 32 issues (6 blocker, 18 major, 8 minor). Each was checked against Blender 5.2.4 where it made a Blender claim (Appendix A rows added in v1.1). All were accepted, three with a different mechanism than the one proposed; none was rejected.

| # | Sev. | Issue | Resolution | Where |
|---|---|---|---|---|
| 1 | blocker | `IdRef<T: IdType>` in L4 crates names types defined in L6 | Accepted. `IdKind` + zero-sized marker kinds in `frap-rna`; `IdRef<K: IdKind>`; `IdType::Kind` ties data to kind; `IdMap<T>` takes `IdRef<T::Kind>`. Frozen at M1 W0 after M0. | §5.1, §5.2, §6.1, §16 |
| 2 | blocker | RNA traits cannot express SoA/derived/computed structs | Accepted. `StructRef`/`CollRef` (borrowed or owned proxy), `RnaMutGuard = Box<dyn RnaMut + 'a>`; bulk `foreach_*` on collections; prototypes (`MeshVertexMut`, derived `MaterialSlot`, embedded path, `Context`) written in M0 WP 0.2 before freezing. | §9.4, §22.2 |
| 3 | blocker | Embedded IDs have no identity | Accepted. `Root::Embedded { owner, slot }`, `EmbeddedSlot`, header without `IdMap` entry, mutation via the owner's `get_mut`; repr rule and verified test string; node-tree `AnimData` and KHR_animation_pointer go through it. | §5.1, §9.3, §18 |
| 4 | blocker | WPs need macros/specs from later or same-wave WPs | Accepted. `rna_struct!`/`#[rna_func]`/`#[rna_hook]` moved to WP 1.2 (W1), operator-tier macros stay in 1.12; mesh element bindings moved out of 1.4 into new WP 1.27 (W2, needs 1.1/1.2/1.4 impl). | §3.2, §22.3 |
| 5 | blocker | `render.render` has no legal path to the GPU | Accepted with a variant: `frap-render-scene` moves to L8 and owns `trait RenderService` (plain data, no wgpu); `frap-render` moves to L9 and implements it; `frap-op` defines `Services` and `OpCtx::services()`; `frap-session` creates the service lazily. The trait sits in `frap-render-scene` rather than `frap-op` so `frap-render` never depends on the scene-facing operator crate. | §3.2, §3.3, §10.1, §12, §19.2 |
| 6 | blocker | Stream can deadlock; no request ids; no persistent agent session | Accepted. `frap serve --jsonl` (`{id, src, on_error}`, complete-unit parsing, `id` echo, done record) and `frap mcp` (one `Session` per connection). Raw stdin kept, and additionally ends a pending continuation on a blank line/EOF. | §11.1, §13.1, §14 |
| 7 | major | Object and mesh names after delete | Accepted. Independent uniquification per `IdMap`; verified (`Cube` / `Cube.001`); T0 golden in WP 1.19. | §5.4, §10.3 |
| 8 | major | Library-linked IDs not modelled | Accepted. Minimal `Library` ID, `ID.library`/`is_linked_packed`, `by_name` keyed on `(name, library)`, local-first lookup, NODES dispatch on (library basename, name), `.frap` persistence. The tuple-key lookup rule did not reproduce in a probe and is pinned by goldens in WP 1.10 before exposure. | §5.1, §5.2, §5.4, §7.2, §15, §16.1 |
| 9 | major | Edit-mode reads mutate `Main` | Accepted. Cached read-only `Arc<Mesh>` view in `EditMesh`, keyed by BMesh generation; reads need no transaction. | §1 #8, §6.3, §9.5, §12 |
| 10 | major | Time applied too early; render/export ignore animation | Accepted. Setter marks time dirty; animation applied at evaluation boundaries (verified behaviour); render and export apply animation on a COW clone; deviation (originals keep manual edits) listed. Golden runner no longer normalises over a dirty time. | §7.3, §18, §19.4, §20.3 |
| 11 | major | Material slots stored per object | Accepted. Slot count derived from data; object stores overrides only, padded lazily; `MaterialSlot` is a proxy. | §5.6, §9.6, §15 |
| 12 | major | Multiple scenes have no home | Accepted. New WP 3.10 in M3: scenes, `bpy.data.scenes.new`, `scene.new/delete`, `context.window` stub (writable `scene`/`view_layer`, verified in `-b`), multi-scene glTF goldens. | §8, §17.2, §22.5, OPERATORS |
| 13 | major | LINES/POINTS, strips/fans, point clouds | Accepted. Extractors for loose edges/points, import conversion of all modes, `PointCloud` ID (`PT`) created by `import_point_as_pointcloud` and exported as POINTS, in M3 (WP 3.7); type exists from M1. | §5.6, §17.2, §22.5 |
| 14 | major | `frap_instancing` duplicates `export_gpu_instances` | Accepted. Dropped; Blender prop with the add-on rule; codegen audit of every `frap_*` option. | §4.2, §17.2, §22.9 |
| 15 | major | KHR_materials_variants has no RNA source | Accepted. Dumps enable `KHR_materials_variants_ui`; add-on preferences exposed and saved; import turns it on. | §4.1, §8, §17.2 |
| 16 | major | Mesh caches can go stale | Accepted. `MeshRuntime::clone` is empty; geometry fields private, writes through `&mut` methods that clear caches; proptest. | §5.3, §6.1 |
| 17 | major | Registration safety net cannot work | Accepted with a variant: the expected set is computed **at test time** from `ops/`/`funcs/` file names instead of a committed `spec/implemented.txt` (a committed list would be a shared file every WP edits); registry refuses duplicates; hook completeness test via `HOOK_DECLS`; runs in release/LTO too. | §4.3, §10.5 |
| 18 | major | Generated spec crate slows every build | Accepted. `frap-spec-rna`, `frap-spec-ops-<group>`, `frap-spec-catalog` (deflated JSON → leaked `&'static` tables); typed structs only for ops with M ≤ `spec/current_milestone`; CI build-time budgets. | §3.2, §4.2 |
| 19 | major | `OpCtx` borrows fight edit operators | Accepted. `Context` no longer borrows `EditState`; `split_edit()`/`split_eval()`. | §8, §10.1 |
| 20 | major | wgpu pitfalls on Windows and start-up | Accepted. Windows: Vulkan first, DX12 with explicit FXC, no `static-dxc`; one uber-shader per engine; `Rgba32Float` accumulation; cold-start time measured. | §3.5, §19.2, §22.3, §22.7 |
| 21 | major | Dev-dependency cycles | Accepted. Rule 2.2.11, `cargo xtask check-deps`, operator tests only in `frap-golden` or `frap-ops-<x>-tests`. | §2.2, §20.1 |
| 22 | major | M1 is not thin; contracts frozen untested; Blender SSIM gate in M1 | Accepted. M0 walking skeleton (0.1) and contract probes (0.2) plus RFC 0001 before M1 W0; Blender Workbench SSIM gate moved to M2 (WP 2.3); M1 renders gate on self-goldens. | §3.4, §22.2, §22.3, §22.4 |
| 23 | major | Hidden same-wave golden dependencies | Accepted. "Golden prerequisite set" rule; M1 re-cut into four waves (prerequisites 1.19/1.22 in W3, consumers in W4; glTF crates 1.16/1.17 accepted at library level in W2, Blender goldens in 1.23); M2 prerequisites (`select_all`, `select_mode`, `frap.select_elements`, edit mode handler) moved into 2.2 (W1). | §22.1, §22.3, §22.4 |
| 24 | major | Acceptance assigned to non-owners; no M6 owner for mode switching; shared glTF file | Accepted. `rotation_mode`/`matrix_world` hooks accepted in 1.10; `MODE_HANDLERS` and `TRANSFORM_MODE_HANDLERS` extension points (6.2 adds files, no reassignment); glTF `PRIMITIVE_EXTRACTORS`/`ATTRIBUTE_WRITERS`/`POST_PROCESS` slices frozen at M1 W0; rule "acceptance follows ownership". | §8, §10.3, §17.2, §22.1 |
| 25 | major | Milestone order | Accepted. 4.0 needs M2; 6.0 needs M3 + M4 (not M5); 5.0 needs M3 + M4; critical path redrawn. | §22.6–§22.11 |
| 26 | major | Uncheckable acceptance tests | Accepted. Differential round-trip law vs Blender's import→export on an allow-list; LUTs tested on held-out colours; `libm` for transcendentals (rule 2.2.12) so transcripts are byte-identical; LSCM within 1e-4. | §17.2, §19.5, §21.3, §21.4, §22.5 |
| 27 | major | No `poll()` / unhelpful poll errors | Accepted. `bpy.ops.<op>.poll()`, `.get_rna_type()`, `.idname()`, `.idname_py()` (verified values) as `OpMeta`; declarative `PollReq`; `detail.requires`/`unmet` + fix `hint`. | §8, §10.1, §11.2, §13 |
| 28 | major | No bulk mesh I/O | Accepted. `foreach_get` (one-argument frap form + Blender form) and `foreach_set` with literal lists; slices; `$array` encoding with `--max-array` paging; nameless `$coll` encoding. | §9.4, §13.3 |
| 29 | major | Errors cascade inside a batch | Accepted. `on_error` per serve/MCP request, default `skip_rest`, `status:"SKIPPED"`; session-level continuation unchanged. | §13.1, §13.2 |
| 30 | minor | Short-form grammar contradictions | Accepted. Path only when a root is followed by `.`/`[`; `D`/`C` routed by what follows the path; any other `ident.ident` is an op call (unknown → `OP_UNKNOWN`), so `frap-lang` needs no operator list. | §11.4 |
| 31 | minor | Radians vs degrees | Accepted. `ANGLE_LIKELY_DEGREES` hint; `ROTATION (radians)` in `--describe`. | §13.2, §14 |
| 32 | minor | `//` export paths, `scale_length`, span units | Accepted. Exit transcripts use cwd-relative paths; frap resolves `//` in export paths (listed deviation, never golden-tested); `scale_length` asymmetry stated; spans are UTF-8 byte offsets plus `col_chars`. | §11.2, §13.4, §17.2, §22.3 |

---

## Decision log (M1 W0, v1.2: RFC 0001 applied, contracts frozen)

WP 1.0 applied every finding of `docs/design/rfc/0001-m0-contract-findings.md` (status: accepted) to this document and to the code, folded the M0 walking skeleton into the full crate tree of §3.2, and froze the §3.4 contracts as compiling stubs. Where RFC 0001 left a choice to the integrator, or the contracts as written could not compile, the decision is recorded here.

| # | Topic | Decision | Where |
|---|---|---|---|
| W0-1 | RFC 0001 blocking findings (F-ED-1/2, F-LINK-1, F-OP-5..9, F-HOOK-1, F-TR-1..4, F-VAL-1..4, F-LAYER-1, F-REN-1/2, F-SES-1) | Applied as proposed: `IdHeader.generation`, `src/generated/`, `frap-scene/src/data.rs`, `deny(unsafe_code)` + per-entry `allow`, `Host::split_edit`/`snapshot_at`, `split_eval → (&dyn HostRead, &mut dyn Evaluator)`, read get hooks returning `Field<'h>`, `RnaHookReg`, `Field::{Embedded, Ptr, NeedsHook}` (`#[non_exhaustive]`), `Item`, `Value::{Id, Structs}` + boxed `Ptr`/`Coll`, `CoerceCx`, `Report` in frap-rna, `GpuOptions`/`RenderRequest` in frap-render-scene, `OpCtx::{new, finish, main_mut, snapshot_at}`, `HostImpl` disjoint from `SessionServices`. | §5.1, §5.2, §9, §10.1, §12, §19.2, §21.6 |
| W0-2 | F-TR-6 (boxing a reborrow) | **Blanket impls** `impl Rna for &mut T` / `impl RnaMut for &mut T`; `RnaMutGuard` stays `Box<dyn RnaMut + 'a>` (the form the probe ran end to end). | §9.4 |
| W0-3 | F-TR-4 (collection items) | **Direct change**: `RnaColl::item` returns `Item<'_>`; no additive `item_any`. | §9.4 |
| W0-4 | F-TR-7 (bulk) | `Bulk { F32, I32, Bool }`, item length from `prop.array_len()`; default `foreach_get`/`foreach_set` loop over items; a wrong length raises Blender's `RuntimeError: internal error setting the array`. | §9.4 |
| W0-5 | F-GEN-1 (const names) | **Module-scoped consts**: struct statics at the crate root (`frap_spec_rna::OBJECT`), property consts in a per-struct module (`frap_spec_rna::object::LOCATION`); codegen fails on any collision and escapes keyword module names. | §4.2, §9.6 |
| W0-6 | F-DEF-2 (`FloatStorage`) | Kept; filled from `spec/overlay/float_storage.toml`, default `F32`. | §4.2, §9.1 |
| W0-7 | `Context::collection()` | Returns `Option<IdRef<CollectionId>>`: the scene's master collection is embedded and has no `IdRef` (`None`), which the bare `IdRef` of v1.1 could not express (e.g. `wm.read_homefile(use_empty=True)`). | §8 |
| W0-8 | `Main.filepath` | `Option<String>`: `camino::Utf8PathBuf` was never a §3.5 dependency and `bpy.data.filepath` is a string. | §5.2 |
| W0-9 | New workspace dependencies | `bitflags 2.13` (F-DEP-1), `proc-macro2`/`quote`/`syn 2` (frap-macros), `toml 1.2` (overlays, `props.toml`, budgets); glam gains its `libm` feature (rule 2.2.12). | §3.5 |
| W0-10 | Same-layer edges | `frap-lang → frap-rna` (L1) and `frap-ops-all → frap-ops-*` (L10) are the only allowed same-layer edges; `cargo xtask check-deps` also checks dev-dependency cycles, `wgpu` only in frap-render, `anyhow` only in frap-cli, no direct `frap-render → frap-scene`, and `workspace = true` on every member dependency. | §3.2, §20.5 |
| W0-11 | Extension-point signatures not spelled out in v1.1 | `ModeHandler { mode: ObjectMode, object_type, enter, exit: fn(&mut OpCtx, &[IdRef<ObjectId>]) -> OpResult }`; `TransformModeHandler { mode: ContextMode, apply: fn(&mut OpCtx, TransformOp, &OpArgs) -> OpResult }`; `MeshExportCx { main, mesh, settings }` and `PrimitiveDraft { mode, material_slot, positions, normals, texcoords, indices, corners }` for the glTF `PRIMITIVE_*` slices. | §10.3, §17.2 |
| W0-12 | Types named but undefined in v1.1 | Defined minimally and frozen as data: `IdProp`, `EvalError`, `BoneRef`, `Preferences`, `VirtualView3D { view_matrix, shading }`, `ObjectMode`, `ContextMode`, `BaseFlag`, `LayerCollection(Path)`, `ViewCache`, `EvaluatedPose`, `ModifierStatus`, `FuncArgs`, `PathResolver`, `ImageCache`, `DecodedImage`, `RCamera`/`RObject`/`RLight`/`RWorld`/`RSettings`, `LoweredMaterial` sub-types, `GoldenState` sub-types, `Request`/`OnError`/`DoneRecord`. Enums likely to grow are `#[non_exhaustive]`; additive changes need no RFC (rule 2.2.4). | crate sources |
| W0-13 | Placeholders for later contracts | `frap_bmesh::BMesh` (contract frozen in M2 W0) and `frap_anim::{Action, AnimData}` (M6) exist as opaque/minimal types so `EditState`, `OpCtx::bmesh_mut`, `NodeTree.animation_data` and `Main.actions` compile now. Later-milestone crates (`frap-uv`, `frap-subdiv`, `frap-boolean`, `frap-codec-mesh`, `frap-modifiers`, `frap-ops-uv`, `frap-ops-anim`) are empty. | §3.2, §6.2 |
| W0-14 | `Response` | Gains `id` (serve/MCP echo), `func` and `edit_mesh` fields to match §13.2; `exec_request` returns `Vec<Response>` and `DoneRecord::from_responses` builds the done record. | §11.6, §13.2 |
| W0-15 | `linkme` retention | Verified on macOS (debug and fat-LTO release): referencing **one** symbol per ops crate (`frap_ops_<x>::link()`) keeps every entry of that crate; `frap_ops_all::link()` calls them all. CI runs the registry test with `CARGO_PROFILE_RELEASE_LTO=fat`. | §10.5 |
| W0-16 | M0 stand-ins | The hand-written M0 `StructDef`/`OperatorSpec` statics moved from `frap_rna::spec_m0` into `frap-spec-rna/src/generated/` and `frap-spec-ops-{mesh,io}/src/generated/` with the W0-5 naming (drift-tested against `spec/m0/`), so WP 1.1's codegen replaces whole directories without touching any `lib.rs`. The M0 operators moved to `frap-ops-mesh/src/ops/primitive_cube_add.rs` and `frap-ops-io/src/ops/export_scene__gltf.rs`; the M0 exit transcript is byte-identical. | §4.2, §22.2 |
| W0-17 | Verifier fixes to W0 (contract amendments before W1 starts) | **glTF primitive contract** (§17.2): W0-11's fixed-field `PrimitiveDraft` could not hold M3/M6 attributes and deduplicated before writers ran, while the add-on's split key includes every per-corner attribute. Now: writers contribute per-corner words to a `SplitKey` before extraction (`PrimitiveAttributeWriter::key`), extractors dedupe on `(vertex, key)` and record `corners`/`vertices`, `PrimitiveDraft` is `#[non_exhaustive]` with an ordered `attributes: IndexMap<SmolStr, AttrColumn>` and `targets`. **Mesh** (§6.1): `vert_normals()`/`bounds()` accessors and caches (body of `vert_normals` WP 1.4). **Helpers/polls** (§10.3, §8): `add_object_with_data`, `frame_range_or_scene`, `iter_edit_meshes` and the named `ED_operator_*` polls declared (bodies WP 1.12); `OpCtx::add_object` changes selection and active only in the context view layer. **`startup::empty()`** declared (WP 1.10); §5.7 states the real `(Main, WindowState)` return type. | §5.7, §6.1, §8, §10.3, §17.2 |
| W0-18 | `Mesh` serialization | No serde derive on `Mesh` in W0: the §15 shape of attributes (`name`/`domain`/`data_type`/`data` with `$buf`) is not what a derive produces, and `frap_rna::serde_buf`/`IdRef` serde are WP 1.2's. WP 1.4 adds serde to `Mesh` (`runtime` never serialized) on top of them; `smallvec` gains its `serde` feature for `DeformVert`. | §6.1, §15 |
| W0-19 | Protocol kinds and codes | The M0 code emits response kind `invalid` and the codes `NOT_IMPLEMENTED` (executor), `RNA_NOT_IMPLEMENTED` and `RNA_RUNTIME` (RNA layer); they are now listed in §13.2/§13.4 instead of only in `deviations.md`. | §13.2, §13.4 |
| W0-20 | Shared files WPs would otherwise need | `fuzz/` is a cargo-fuzz workspace excluded from the root one (targets `lang_parse`, `lang_split` owned by WP 1.3; CI checks they compile on stable); `xtask` depends on `flate2` (WP 1.1's `catalog.json.deflate`); `frap-schema` dev-depends on `frap-ops-all` and `tests/describe_ops.rs` snapshots every registered operator as `tests/snapshots/op__<idname>.snap` (M0 ops blessed with a minimal fragment; WP 1.18 re-blesses with the §14 shape); `frap-io-gltf` depends on `frap-geom` (WP 1.17 welding); the transcript runner globs `tests/cli/**/*.{trycmd,toml}` (fixtures in `*.in/`/`*.out/` skipped); `frap-mesh/src/primitives/` is an `automod` directory; rule 2.2.12 and the §3.2 `frap-render` type ban are enforced by per-crate `clippy.toml` files. | §3.2, §4.2, §20.1, §22.1, rule 2.2.12 |
| W0-21 | Verifier fix: datablock shapes (§5.6, §22.1 "Needs … always satisfied by Wave 0") | W2 packages (1.13, 1.15, 1.16, 1.17, 1.11) need the datablock fields that WP 1.10 implements in the same wave. W0 now declares the §5.6 field sets: `Object` (all three rotations, `delta_*`, `parent`/`parent_type`/`parent_bone`/`parent_vertices`, `matrix_parent_inverse: Mat4`, `modifiers: Vec<Modifier>`, `material_slot_overrides: Vec<SlotOverride { link: SlotLink, material }>`, `active_material_index`, `pose`, `hide_*`, display fields, `show_*`, `animation_data`), `Scene` (embedded master `collection: Collection`, `camera`, `world`, `frame_start/end/current/step`, `render`, `view_settings`, `display_settings`, `display`, `eevee`, `cursor`, `tool_settings`, `unit_settings`), `Collection` (`hide_*`, `color_tag`), `Material` (embedded `node_tree`, viewport `diffuse_color`/`metallic`/`roughness`, …; the M0 flat Principled values are gone and the M0 exporter reads the factory tree's Principled inputs), `Camera { fields: CameraFields }`, `Light { type, fields: LightFields }` (union of the per-type fields), `World` (embedded `node_tree`, `color`), `Image` (metadata, `EncodedImage`), `Modifier` (§7.1, settings a `#[non_exhaustive]` stand-in until M4 W0) and a placeholder `Pose` (M6 W0). Constructors use values verified on Blender 5.2.4 (`*.new()` fresh values; factory overrides in `startup.rs`). Generated plain sub-structs (`CameraFields`, `CameraDOFSettings`, `LightFields`, `RenderSettings`, `ImageFormatSettings`, `ColorManaged*Settings`, `SceneDisplay`, `SceneEEVEE`, `UnitSettings`) are hand-written stand-ins in `frap-scene/src/generated/` that WP 1.1 regenerates with the same names and field types. `NodeTree::new(name, ty, embedded)` added. | §5.6, §7.1 |
| W0-22 | Verifier fix: `Mesh.from_pydata` RNA function | `RnaFuncImpl` and `RNA_FUNCS` live in frap-scene (L6), which frap-mesh (L4) cannot reach, and frap-rna's traits have no struct-local call hook. The Python-only `Mesh.from_pydata/shade_smooth/shade_flat` RNA functions move from WP 1.27 to WP 1.22 (`frap-ops-data/src/funcs/mesh_*.rs`, over the WP 1.4 Rust API); 1.27 keeps the bindings and the bulk `foreach_*` overrides (which are `RnaColl` methods). Rule stated in §9.5: global registrations live at L6 or above. | §9.5, §22.3 |
| W0-23 | Verifier fix: stubs in `automod` directories | §22.1 now exempts per-concept files in `automod` directories from W0 stubbing (`ops/primitive_*.rs`, the `frap-ops-object` op files, `modes/object.rs`, `material__new.rs`, `transform` ops, `render__render.rs`): the directories exist with `_dir.rs`, and an empty op file would fail the §10.5 registry test. Every non-`automod` module named in §22.3 exists. | §22.1 |
| W0-24 | Verifier fix: session options and response shape | `SessionOptions` is `#[non_exhaustive]` (construct from `Default`, assign fields) with `cwd`, `gpu`, `startup: Startup { Factory, Empty, File }`, `verbosity: Verbosity { Minimal, Normal, Full }`, `max_items`, `max_array`, `limits: Limits { max_elements, stmt_timeout }`, `fail_fast`; `Session` keeps them (`Session::options()`). `Response.context` is `Option<J>` (absent with `--verbosity minimal`). §13.2 now states that a `set` response echoes the coerced value in `value` (without `rna`), as §9.6 and the frozen M0 transcript do. | §12, §13.1, §13.2 |
| W0-25 | Verifier fix: `placement` signature | The text now matches the frozen code: `placement(cx, &location, &rotation, align: Align, &scale) -> Result<Placement, OpError>`; `align` by value because Blender uses only its value. | §10.3 |
| W0-26 | Verifier fix: test dependencies and test crates | Dev-dependencies added so WPs never edit a `Cargo.toml`: `proptest`, `insta`, `approx`, `serde_json` on every M1 `frap-ops-*` crate, `frap-op` (plus `frap-spec-rna`, `frap-spec-ops-{mesh,object}`), `frap-render-scene`; `approx`/`serde_json`/`proptest`/`insta` where missing on frap-rna, frap-mesh, frap-scene, frap-nodes, frap-material, frap-eval, frap-io-gltf. New test-only crate `crates/frap-ops-io-tests` (layer 12 in `check-deps`, owned by WP 1.23) for the save/open proptest that needs a `Session`. WP 1.12's dummy-op tests are integration tests in `crates/frap-op/tests/`. WP 1.17 compares its in-process import against the dump JSON (it cannot dev-depend on `frap-golden`). | §22.1, §22.3, rule 2.2.11 |
| W0-27 | `codegen --check` and `budgets` before WP 1.1 | Both pass vacuously until WP 1.1 generates code and records baselines; they are non-gating until then and say so with a `::warning` annotation on GitHub Actions (`xtask::ci_warning`). | §4.2, §20.5 |
| W0-28 | Second verifier pass on W0 | **Ownership**: `xtask/Cargo.toml` and `xtask/src/budgets.rs` belong to WP 1.0; in M1 W1, WP 1.1 and WP 1.8 may append `foo.workspace = true` lines to `xtask/Cargo.toml` for dependencies already in `[workspace.dependencies]` (append-only), and WP 1.1 writes the `xtask/budgets.toml` baselines with `budgets --record`. **insta** gains `glob`, `yaml`, `redactions` (WP 1.3 corpus, WP 1.18 per-op snapshots). **Messages**: `RnaError::into_frap` drops the prefix's trailing space when the body is empty (`TypeError: bpy_struct: item.attr = val:`, `Converting py args to operator properties:`, verified on 5.2.4); the clamp INFO report has one wording, `frap_rna::clamp_report` (§9.2, D-03), used by both the operator pipeline and assignments. **Extension points**: `MATERIAL_PATTERNS` (`MaterialPattern { name, order, lower, raise }`, `LowerCx`, `RaiseCx`, the last two `#[non_exhaustive]`) so M3 adds pattern files without editing `lower`; `MeshExportCx` is `#[non_exhaustive]`, built with `MeshExportCx::new`, and carries `object: Option<IdRef<ObjectId>>` for M6 skinning and object-linked slots. **Gaps** recorded in `deviations.md` §2 with owners and closing acceptance: `OP_UNKNOWN` for known-but-unimplemented operators (WP 1.1/1.12), no factory `World` (WP 1.10), missing internal `.select_*`/`.uv_select_*` attributes on primitives (WP 1.4). **Vacuous W0 gates and `todo!()` stubs** (`codegen --check`, `budgets`, `Session::exec_request`, `DoneRecord::from_responses`, `IdMap::rename`, `helpers::frame_range_or_scene`/`iter_edit_meshes`) panic or pass vacuously until their WP lands; WP 1.1, 1.10, 1.12 and 1.14 acceptance makes them real. | §3.4, §9.2, §16.2, §17.2, §22.3 |

## Decision log (M1 W1 integration)

The nine W1 packages (1.1 to 1.9) were merged into `m1-vertical-slice` with `git merge --no-ff` in the order 1.2, 1.1, 1.3, 1.4, 1.5, 1.6, 1.9, 1.7, 1.8. There were no textual conflicts. Every change a WP made outside its §22.3 paths, and every addition to a frozen contract, is recorded here. All of them are additive unless stated otherwise. Behaviour that differs from Blender is in `deviations.md`.

| # | Topic | Decision | Where |
|---|---|---|---|
| W1-1 | WP 1.1 codegen output | New public API, with no `lib.rs` edits: `frap_spec_rna::{find_struct, ALL_STRUCTS, BPY_STRUCT_FUNCS}`; `frap_spec_ops_<group>::ALL_OPS`; `frap_spec_catalog::generated::{find, contains, is_typed, CATALOG_IDNAMES, TYPED_IDNAMES}`; `frap_scene::generated::{plain::*, modifier_settings::{ModifierType, ModifierSettings}}`; `frap_nodes::generated::{NodeTypeDef, SocketDef, SocketDefault, NodePropDefault, node_type, NODE_TYPES}`. The W0 plain structs keep their names and field types and gain every other writable dumped field. ID-pointer fields are `Option<IdAny>` with `#[serde(skip)]`. Dynamic enums are `SmolStr` in typed props. A props struct gets a namespace prefix only when the same op name exists in two namespaces of one group (`ExportSceneGltf`/`ImportSceneGltf`). The M0 drift tests `frap-spec-{rna,ops-mesh,ops-io}/tests/spec_m0_drift.rs` are replaced by `tests/drift.rs`. `frap-schema/tests/snapshots/op__export_scene.gltf.snap` was re-blessed (data only: 110 generated props instead of 2), and the integrator accepts it. | §4.2, §5.6, §22.3 |
| W1-2 | Budgets | `xtask/budgets.toml` `[baseline_secs]` are provisional values from an M1 Max, not from CI. The CI `budgets` step stays non-gating (`continue-on-error`) until it is re-recorded on ubuntu-24.04 with `cargo xtask budgets --record`. `cargo xtask codegen --check` gates from now on. `cargo xtask spec-refresh --check` is not wired yet (open). | §4.2, §20.5 |
| W1-3 | WP 1.2 RNA core | Additive: `frap_rna::value::coerce_element` for single-element writes (`prop[i] = x`, `.x = x`). `coerce` now handles pointers, collections and enum flags. `RnaError::into_frap` also drops the prefix's trailing space when the message starts with `,` or `:`, and there is a new `join_prefix`. `traits.rs` adds `ToRnaValue`, `FromRnaValue`, `IdField`, `AsEulerOrder`, `RnaStruct`, `VecColl[Mut]`, `IdVecColl[Mut]`, `vec_item_type`, error helpers and a `SmolStr` re-export for macro expansions. `testing.rs` adds `BindingKind`, `BindingDecl`, `BindingCheck`, `SoaItems`, `RnaBindings`, `check_bindings` and `binding_problems`. `naming.rs` adds `unique_id_name`, `unique_subname` (`BLI_uniquename_cb`) and `truncate_utf8`. `unique_name` keeps its signature and follows the `main_namemap` algorithm. `IdCode::from_code` is new. | §9 |
| W1-4 | `.frap` ID and buffer serde | `frap_rna::serde_buf` wire format: `{"$buf": id, "dtype": "f32x3", "len": n}`, with an inline array when the sink returns `None` (`BufElem`, `BufferSink`, `BufferSource`, `with_sink`, `with_source`, an `option` submodule). The new module `frap_rna::serde_id` (`IdRefNamer`, `IdRefResolver`, `IdNameTable`, `with_namer`, `with_resolver`) gives `IdRef<K>`/`IdAny` serde. Save-side naming uses `IdRefNamer`, which writes the library **name**, because two libraries can share a filepath. `IdNames` returns the library **filepath**, for repr. WP 1.23/frap-project installs the namer and resolver around save/load; a resolver that allocates keys on first sight handles forward references. | §9.3, §15 |
| W1-5 | WP 1.2 math | Additive frap-math API: the `euler::*` order conversions and `compatible_eul`, `EulerOrder::{ALL, rotmode}`, w-first `quat::QuatWxyz`, `matrix::{decompose_wxyz, mat4_to_loc_rot_size, mat3_to_rot_size, loc_*_size_to_mat4, RotationMode, RotationValues, rot_mode_change_values, ObjectTransform}`, a full `BliRng` plus `array_randomize`/`hash_frand`, and `fmt::{shortest, shortest_f64, is_json_integer}`. The `decompose` signature is unchanged. Pending for WP 1.10: the `Scene.frame_current` hook must reproduce Blender's setter (5.2.4 reads back 1000000000 after `= 1000000000` and 0 after `= -1000000000`), not the generic hard-range clamp. | §9, §5.6 |
| W1-6 | WP 1.3 language IR | `PathSeg::KeyLib(String, Option<String>)` is a new variant on a `#[non_exhaustive]` enum, for `bpy.data.node_groups['Smooth by Angle', None]` (§11.2 updated). WP 1.10/1.14 must resolve it explicitly. In `split.rs`, `Piece.unclosed` is replaced by `Piece.open: Option<Open>` plus `bad_close`, `too_deep` and `empty`; `MAX_BRACKET_DEPTH = 200`; there is a `RawSplitter`/`Chunk` raw-stdin accumulator. `lex()` returns `Vec<Token>` and never fails (`Tok::Error`). `Tok::Op2` is renamed `Tok::Op`, and `Token.err_at` is new. Only frap-lang and the fuzz targets use these. | §11.2 |
| W1-7 | WP 1.3 errors | Code `NAME` (NameError) is now raised by frap-lang, not the executor (§13.4 updated), for a lone identifier or an unknown call (`x`, `cube_add()`), which used to be `SYNTAX` (§13.4 updated). A bare expression (`path * 2`, literals, `(…)`, `True`) is `UNSUPPORTED_SYNTAX` with a `print(...)` hint. `;;` gives one `SYNTAX` result per empty statement. All three codes are registered in `frap_rna::ERROR_CODES` via `#[linkme(crate = frap_rna::linkme)]`. Parsing a statement nested deeper than about 24 levels runs on a short-lived scoped thread with a 64 MiB stack. §11.3 is updated for these errors. The short-form built-in calls and the `data.`/`context.` roots inside bpy expressions are covered by the frap-lang corpus snapshots (`crates/frap-lang/tests/corpus/`); writing them into §11.3/§11.4 is still open. | §11, §13.4 |
| W1-8 | WP 1.4 `frap-mesh/src/lib.rs` | The integrator accepts the one-line `pub use` WP 1.4 added to this integrator-owned file. It re-exports `NormalsDomain` and the attribute-name constants `SHARP_EDGE`, `UV_SEAM`, `CUSTOM_NORMAL`, `REMOVED_WHEN_ALL_FALSE`, `SELECT_*` and `UV_SELECT_*` from the private `mesh.rs`. | §6.1 |
| W1-9 | WP 1.4 Mesh behaviour (§6.1 updated) | `AttributeSet` is in creation order, which matches Blender 5.2.4. `Mesh::new` creates no attributes. `set_bool_attribute` is a plain insert, matching the RNA proxy and attribute-write semantics; only `remove_all_false_flags` (`bm_to_me`) trims all-false `sharp_face`/`sharp_edge`/`uv_seam`, and WP 1.27, WP 1.22 and the M2 BMesh conversion rely on this. `normals_domain` and corner normals read converted flags of any type or domain (`lookup_bool`, `sharp_face_flags`, `sharp_edge_flags`). The frozen `sharp_faces()`/`sharp_edges()` stay raw stored-BOOLEAN reads. Quad looptris follow `is_quad_flip`. Additive API: `positions_mut`, `edges_num`, `bool_attribute`, `looptri_faces`, `calc_edges`, `from_pydata`, `shade_flat`/`shade_smooth`, `AttributeSet::{get_mut, iter_mut, len, is_empty}`, `AttrData::{default_of_len, gather_with_default}`, `Domain::identifier`. Primitives: one struct per primitive with Blender's operator property names and defaults plus the creation matrix (`primitives::creation_matrix`, a port of `ED_object_new_primitive_matrix`, and `scale_matrix`). `Torus::radii` clamps `EXT_INT` radii to [0, 10000]. `tools/blender/dump_primitives.py` runs with `-t 8`. The M0 `primitives::cube` signature is kept. | §6.1 |
| W1-10 | W0-18 still open | `Mesh` serde for `.frap` (§15 attribute shape, `smallvec` serde for `DeformVert`) is not done yet. Its prerequisites `serde_buf` (W1-4) and the frap-project container (W1-12) are now merged, so it is scheduled as a WP 1.4 follow-up before WP 1.23. | §6.1, §15 |
| W1-11 | WP 1.5 glTF crate | `frap-gltf/Cargo.toml` gains `linkme` for the `json::EXTENSIONS` slice (rule 2.2.1). `BufferBuilder` keeps `pub bytes` and gains `buffer`, `views`, `accessors`, `view_base`, `accessor_base` and methods. `Document.extras` is `Option<Extras>` (a JSON-value-equal newtype over `Box<RawValue>`). `Extensions` compares numbers by value. Document fields follow io_scene_gltf2's top-level key order; other structs use its alphabetical `to_dict` order. New modules: `error.rs`, `primitive.rs`, `json/extensions.rs`. `validate::validate_file` (re-exported) and `json_text_issues` are new. Known limits: unknown enum values fail the parse, naming the value, instead of loading with `VALUE_NOT_IN_LIST`; unknown non-extension members are reported but not kept on write; extension integers wider than 64 bits are read as f64; a known member written as `null` is reported as `UNEXPECTED_PROPERTY`. | §17.1 |
| W1-12 | WP 1.9 `.frap` container | Public API: `frap_project::{container, manifest, buffers, migrate}` (see the module docs). Format choices §15 left open: manifest entries are `{hash, size}` for buffers and `{hash, ext, size}` for images; a packed image in `scene.json` is `{"$img": hash, "ext": "png"}`; schema 0 is the §16 proposal draft. The W0 `migrate/steps/_dir.rs` placeholder is deleted now that the directory has `v0_to_v1.rs`. | §15, §16 |
| W1-13 | WP 1.6 images and colour | `ImageError::Io(String)` (`#[non_exhaustive]` enum). §19.1 now calls `frap_color::display_transform(&film.rgba, film.width, film.height, &ViewSettings, &DisplaySettings)`: frap-color (L1) cannot name `Film`. WP 1.24 converts with `DisplayImage::to_rgba8(render.dither_intensity)` or `to_rgba16()`; images are top row first. `ColorError::NotAvailable` is returned for view, look and display combinations that are not baked yet; the caller falls back to Standard with a WARNING. New frap-color items: `ViewTransform`, `DisplayDevice`, `DisplayProcessor`, `look_applies`, `ColorSpace`, `lut::*`, `delta_e`. `serde_json` is a dev-dependency of frap-color and frap-imbuf. | §19.1 |
| W1-14 | WP 1.8 golden harness ownership | WP 1.8 owns `tests/golden/{cases,fixtures}/harness/**` (the three seed cases), `tools/blender/test_golden_runner.py` and `crates/frap-golden/rna_fallbacks.txt` (§22.3 updated). `frap-golden/Cargo.toml` gains `png` and `frap-nodes`, which check-deps allows. In `state.rs`, only the body of `locate_blender` changed (it delegates to `blender::locate_from`, and `USERPROFILE` is accepted as HOME). The canonical glTF form writes node transforms as a column-major `matrix` and primitive modes as POINTS/LINES/TRIANGLES. `xtask golden live` passes `--include-ignored`. WPs 1.10, 1.14 and 1.27 delete their `rna_fallbacks.txt` lines when they bind those properties. On CI the `golden_runner_python_unit_tests` test needs `python3`, or `FRAP_GOLDEN_NO_PYTHON=1`. | §20, §22.3 |
| W1-15 | WP 1.7 renderer conventions | `RObject::random_hash` is `BLI_ghashutil_strhash_p_murmur(id.name)`: MurmurHash2, seed 0, over the UTF-8 bytes `"OB" + name + NUL`. The renderer applies `BLI_hash_int_01` and HSV. A reference implementation is in `crates/frap-render/tests/workbench_ref.rs`, and WP 1.15 copies it. `frap-render` dev-depends on `frap-material`. New: `Gpu::read_texture_in_bands`, `readback::{band_rows, READBACK_BAND_BYTES}`, and the `workbench` module. `gpu.yml` sets `FRAP_RENDER_TIMINGS` and uploads the cold-first-render timings of each adapter as an artifact. **Open contract gap (WP 1.15 + integrator, M1 W2):** Blender's Workbench MATERIAL colour reads `Material.diffuse_color/metallic/roughness`, and its background reads `World.color`. frap reads the Principled factors and the World Background node. Fixing it needs an additive per-slot workbench `[rgba, metallic, roughness]` and a world viewport colour in `RSettings`/`RWorld` (deviation D-22 until then). **Closed in W2 (W2-7).** | §19.2 |
| W1-16 | Shared target directory | Worktrees sharing one `CARGO_TARGET_DIR` reused each other's rmeta files and gave false E0425/E0432 errors. The integration gate ran on the merged tree only, so it is not affected. Future waves should give each WP worktree its own package profile override, or run the gate in the main tree only. | §22.1 |

## Decision log (M1 W2 integration)

The ten W2 packages (1.10 to 1.18 and 1.27) were merged into `m1-vertical-slice` with `git merge --no-ff` in the order 1.13, 1.12, 1.14, 1.10, 1.11, 1.15, 1.16, 1.17, 1.18, 1.27 (1.14 before 1.10, 1.11 before 1.15, as the WPs asked). There were no textual conflicts; `Cargo.lock` merged cleanly and `cargo metadata --locked` accepts it. All changes below are additive unless stated otherwise. Behaviour that differs from Blender is in `deviations.md` (D-03 extended, D-15 rewritten, D-21, D-23 to D-40; D-22 closed; four §2 gaps closed, nine added).

| # | Topic | Decision | Where |
|---|---|---|---|
| W2-1 | WP 1.10 scene contracts | `ViewLayer: PartialEq`; `Context::{selectable_objects, editable_objects, base_visibility}` and `pub mod rna` (`context/rna.rs`). `data.rs`: `IdMap::contains`, `IdMap::insert` sets `header.session_uid` from the `IdKey`; `Main::{library_by_filepath, find_key_lib, rename, all_ids, for_each_user_ref, user_counts, users, orphans, remap, remove, embedded, embedded_header, embedded_mut}`. New public modules `id::{layers, slots, transform, paths, idlist}`; `Library::new`, `Light::srna_of`, `Scene::{UNCLAMPED, frame_from_input}`. The W0 placeholders `frap-scene/src/{funcs,hooks}/_dir.rs` are deleted. Startup is built from `spec/blender-5.2/startup.json` (factory `World`, the Layout `view_matrix` and shading in `WindowState.view3d`); `startup::empty()` is implemented. `view_layer.objects.selected` lists every selected base, unfiltered by visibility, as Blender does. | §5.2, §5.7, §8 |
| W2-2 | WP 1.10 `check_bindings` | `Light` and the generated plain settings structs have no `RnaBindings` impl, so `crates/frap-scene/tests/bindings.rs` checks them with value-shape reads instead of `check_bindings`. Accepted as WP 1.10's acceptance evidence for those types. | §9.5 |
| W2-3 | Linked IDs: order, tuple key, repr (ruling) | Linked IDs sort after local ones. The tuple key uses the **first** library with the given filepath, as Blender's lookup does; an ID of a second library with the same filepath (the packed `….blend.001` archive) therefore cannot be addressed by tuple key, and its shadowed repr does not resolve back. Linked IDs repr as plain `['Name']` unless a local ID shadows them (then `['Name', '<filepath>']`, D-15). `frap_rna::ptr::{IdNames, id_repr}` docs updated. Linked tuple-key goldens (filepath key, local-before-linked lookup, sort order) are added by WP 1.21/1.26 once `object.shade_auto_smooth` exists; until then unit tests cover them. The session's `bpy.data.<coll>[name, lib]` step now delegates to `Main::find_key_lib` (integration fix: WP 1.14 had its own lookup with another message). | §5.2, §5.4, App. A |
| W2-4 | WP 1.11 nodes and lowering | `tree.rs` re-exports the new `tree/{build,edit,avail,toposort}.rs` API (`Node::{from_type, input*, output*, set_prop, update_availability, …}`, `NodeTree::{new_node, add_default_material_graph, add_default_world_graph, link_new, remove_node, toposort_left_to_right, links_valid, …}`, `SocketAddr`, `find_socket`, name/limit constants). New extension slice `frap_nodes::NODE_AVAILABILITY` (one `availability/<node>.rs` per rule, registered with `#[linkme(crate = frap_rna::linkme)]`). The `NodeTree` RNA bindings are real (`frap_nodes::rna` proxies `NodeRef/NodeMut`, `SocketRef/SocketMut`, `LinkRef/LinkMut`, collections). `for_each_ref`/`remap_ref` cover ID socket defaults and `animation_data.action`. `Node.is_active_output` reads as the first output node of its type. `frap-material`: `RaiseCx` gains `bsdf`, `output`, `base_alpha_socket` (end of the clip chain), `locations`; `LoweredMaterial::{gltf_default, patterns_in_order}`; public `build` (with `LAYOUT_BLOCKS` slice) and `graph` modules. `LoweredMaterial` uses glTF semantics (emissive factor ≤ 1 with `emissive_strength` ≥ 1; `base_color.factor[3]` is alpha; unrecognised graphs lower to the glTF default with a WARNING); `raise` rebuilds the whole embedded tree the way the importer does. Links past an input's `link_limit` stay valid (verified on 5.2.4). | §7.2, §17.1 |
| W2-5 | WP 1.12 operator framework | `frap-op/Cargo.toml` depends on `frap-spec-catalog` and every `frap-spec-ops-*` (KnownNotImplemented, §10.1); check-deps passes. New `frap_op::pipeline` (macros run through `pipeline::execute`; `OP_NOT_IMPLEMENTED` maps to `OpError::NotImplemented`); `PollReq::{code, text}` + `Display`; `Registry::{build, try_global, typed_specs, get, spec, spec_by_bl_idname, get_by_bl_idname, known_idnames}`. `OP_POLL_FAILED` `detail.requires/unmet` are now requirement codes (`MODE_EDIT_MESH`), matching the manifest (closes WP 1.18's mismatch; `docs/agents.md` updated). `OBJECTMODE.requires = [SceneEditable, ModeObject]`; new polls `ACTIVE_EDITABLE`, `ACTIVE_LOCAL_EDITABLE`, `EDITMESH_VIEW3D`, `SELECTED_OBJECTS`, `SCENE_HAS_CAMERA`, `LIBRARY`. Placement: an unset `scale` gives 1, a set value is used as is (Blender's verified rule; M0 mapped (0,0,0) to 1). Known-but-unimplemented operators now raise `OP_NOT_IMPLEMENTED`, and `align='VIEW'/'CURSOR'` works (§2 gaps closed). | §10 |
| W2-6 | WP 1.13 evaluation | `frap-eval` gains `libm` (rule 2.2.12); `lib.rs` re-exports `cache::{EvalCache, bound_box, dimensions, matrix_basis, object_transform, parent_matrix, scale_for_dimensions, world_matrix}`; new `EvalCache::{fingerprint, len, is_empty, clear}` and `frap_eval::animate::{AnimationApplied, apply_pending_animation, apply_animation_at}`. | §7.3 |
| W2-7 | WP 1.15 render-scene builder; W1-15 gap closed | Integrator sign-off for commit `486b3a6`: `RSettings` gains `workbench_materials: IndexMap<MatKey, RWorkbenchMaterial>` and `workbench_background: Option<[f32; 4]>` (`Default`: empty, `None`); new `RWorkbenchMaterial { diffuse_color, metallic, roughness }` (§19.2 block updated). `frap-render`'s Workbench `surface()` prefers them in MATERIAL mode and `background()` returns the viewport background when set. D-22 is closed. `frap-render-scene` depends on `frap-nodes` and `libm`. VIEWPORT `background_type` uses `shading.background_color` even without a world; a scene without a world renders transparent black. `GpuCache` hash limits are documented, not checked. | §19.2, §19.3 |
| W2-8 | WP 1.14 session and protocol | `frap-session` depends on `frap-math` and `toml` (embeds `spec/overlay/compat_4x.toml`). `resolve` rewritten around `Target::{Data, Context, Struct, None, Prop, Coll, List, Func, Value}`; `srna_of`/`item_type` removed (crate-internal users only). New `Session::{host, state_dump, render_service_started}`, `DoneRecord::to_json`, code `ZERO_DIVISION` (D-25). **`ErrorCodeDoc` ownership:** frap-session registers the RNA codes it raises (`RNA_*`, `CONTEXT_READONLY`, `NOT_IMPLEMENTED`, `ZERO_DIVISION`, `INTERNAL`); frap-rna must not register them as well. **§13.3 truncation:** collection slices other than attribute data stay plain arrays cut at `--max-items` without a marker until M2 decides the shape (§2 gap). Integration fix: a set hook's `TypeError` (`PROP_ID_SELF_CHECK`, pointer type, `None` refusal) now carries the assignment prefix `bpy_struct: item.attr = val: `, as Blender's `pyrna_py_to_prop` does (verified for `Object.parent`, `Object.data`, `ViewLayer.active_layer_collection`). | §9, §13 |
| W2-9 | WP 1.18 schema export | Per-namespace tools take `call: anyOf[{op: const, args}]` instead of an `op` enum plus `oneOf` (provider schema subsets reject `oneOf`; §14 updated). New code `DESCRIBE_TARGET` (KeyError, `detail.did_you_mean`, §13.4 row added), new target `tools-mcp`, `DescribeOptions::tool_namespaces` (CLI flag name left to WP 1.25). Sign-off for `frap-schema/Cargo.toml`: deps on every `frap-spec-ops-*`, `frap-nodes`, `frap-math`, dev-dep `frap-session` (check-deps OK). `--describe=protocol` gains `frap_project_manifest`. Schemas omit a dumped `default` that is not a valid value; the manifest keeps it. `bpy.ops.frap.describe` / `get_rna_type` routing into frap-schema has no M1 owner yet: `docs/agents.md` points to `frap --describe=<target>` (WP 1.25). | §13.4, §14 |
| W2-10 | WP 1.16 glTF export | `ExportError` is `#[non_exhaustive]` with `Invalid(String)`; `ExportOutput` gains `files`, `reports`, `document`; `ExportFormat::GltfEmbedded`; `ExportSettings` gains the Blender-named fields plus frap-only `file_stem`, `blend_dir`, `from_props`, `settings::SUPPORTED_PROPS`; new public `export::{settings, vtree, nodes, gather, images, materials, write, mesh::{convert, gather}}`. The M0 operator `export_scene__gltf.rs` (WP 1.23's file) has a minimal guard refusing every `export_format` but `GLB` with `NotImplemented`; WP 1.23 removes it and gates `GLTF_EMBEDDED` on the add-on preference (§4.1). | §17.2 |
| W2-11 | Test-file ownership | WP 1.16 owns `crates/frap-io-gltf/tests/{export_*.rs, export_support/**, snapshots/export_*}`; WP 1.17 owns `crates/frap-io-gltf/tests/import_*.rs` (§22.3 updated). | §22.3 |
| W2-12 | WP 1.17 glTF import | New public `frap_io_gltf::import::{import_file, import_bytes, ImportSettings, ImportError, ImportReport, EXTENSIONS_MANAGED, …}`; WP 1.23's operator calls `import_file(main, win, path, &settings, log)`. `material_graphs_match_blender` is no longer ignored; it skips the two `KHR_materials_unlit` dumps until WP 3.5 raises the unlit pattern (§2 gap). | §17.2 |
| W2-13 | WP 1.27 mesh RNA | Only `crates/frap-mesh/src/rna/**` changed. Unbound Mesh/element properties that exist in the `StructDef` now raise `RNA_NOT_IMPLEMENTED` instead of `RNA_NO_ATTRIBUTE` (other structs keep the `rna_struct!` default). Deviations D-34 to D-40. Open: `Float4AttributeValue` dump (WP 1.1) for typed `FLOAT4` data; rustdoc link `sharp_face` in `frap-mesh/src/mesh.rs` (WP 1.4). | §9.5 |
| W2-14 | Golden case rewritten | `tests/golden/cases/scene/select_hidden.py` created its base-less object with `bpy.data.objects.new`, which is WP 1.22 (W3); the §22.3 golden prerequisite set lets W2 goldens use only W2 calls. The integrator rewrote it to get a base-less object by excluding its collection (same `select_set`/`hide_set` semantics, verified on 5.2.4), re-blessed the fixture and updated `crates/frap-scene/tests/golden_replay.rs`. `crates/frap-scene/tests/idmap.rs` now expects the material tree's real srna `ShaderNodeTree` (the W0 stub said `NodeTree`). | §20 |
| W2-15 | Shared target directory, again | Test binaries built by other worktrees in the shared `CARGO_TARGET_DIR` were reused by the merged tree (cargo's dep-info paths are relative and mtimes looked fresh), so tests read data through `CARGO_MANIFEST_DIR`s of other worktrees. The gate was run after removing the worktrees and touching every tracked `.rs`/`Cargo.toml`, which forces a rebuild of all workspace crates. Future waves should give each worktree its own target dir or run the gate only after this reset. | §22.1 |
| W3-1 | WP 1.19 primitives and object prerequisites | Integration sign-off. `frap-session/src/resolve.rs` (WP 1.14): `bulk_json` returns a flat array when the bulk values do not split into whole items (a plane's `polygons.foreach_get('vertices')` was cut to one triangle, shape `[1,3]`), with a unit test. The W0 placeholders `frap-ops-object/src/{ops,modes}/_dir.rs` are deleted. Shared primitive helpers live in `frap-ops-mesh/src/ops/primitive_cube_add/common.rs`, a submodule of an owned file (§22.1); automod and the registry test skip subdirectories. New/updated `op__*.snap` snapshots (12 new operators, `mesh.primitive_cube_add` notes) and `docs/OPERATORS.md` checkboxes. New CLI transcript `tests/cli/m1/object_prereq/`. Fixtures under `tests/golden/fixtures/<area>/**` belong to the owner of `tests/golden/cases/<area>/**` (also for WP 1.22's `data`). The `select_all` and `mode_set` goldens live in `cases/object/delete/` (WP 1.20 owns `object/select`). | §6.1, §10, §22.3 |
| W3-2 | WP 1.22 data API core | Integration sign-off for the contract changes. `frap-rna`: `RnaPtr` gains `pub detached: Option<Detached>` with `pub enum Detached { Evaluated, Temporary }` (constructors set `None`; `PartialEq` compares it, so `ob.evaluated_get(dg) != ob` as in Blender); `coerce` accepts any tuple/list for `PropKind::Collection{item_type:"AnyType"}` parameters of Python-only functions (untyped `Value::Tuple`, else `object of type 'int' has no len()`). `frap-session`: `resolve::step` wraps `step_inner` and propagates `detached` (a re-rooted ID becomes `Temporary`), binds `ID.original` and `ID.is_evaluated` for every ID; `resolve::write::refuse_detached` refuses property writes and `foreach_set` through detached pointers (`NotImplementedError`/`NOT_IMPLEMENTED`); `session/funcs.rs::bind_args` prefixes keyword coercion errors with `error with keyword argument "x" - ` (Blender's `pyrna_func_call`), and Python-defined methods (`PYTHON_ONLY` functions outside `BPY_STRUCT_FUNCS`, e.g. `Mesh.from_pydata`) get CPython's arity wording through `cpython_arity` and no prefix. `frap-ops-data/src/funcs/_dir.rs` deleted. Deviations D-41 (evaluated/`to_mesh()` results are read-only views), D-42 (`user_clear`), D-43 (copy of a selected object comes back unselected). **Open:** RNA functions of other WPs (`select_set`, `vertex_groups.new`, `keyframe_insert`, …) called through a detached pointer still act on the original; a generic guard needs a per-function `mutates` flag (WP 1.14/1.1 follow-up; real evaluated copies with WP 4.1). D-43 needs per-object base flags (WP 1.10 follow-up) before W4 duplicate goldens rely on copy selection. | §5.5, §7.3, §9.2, §9.3, §9.5 |
| W3-3 | WP 1.25 CLI, serve and MCP | New code `REQUEST_INVALID` (ValueError, registered by `frap-cli/src/serve.rs`, §13.4 row added). `frap serve --jsonl` prints the ready/end events like raw mode; MCP stdout carries JSON-RPC only; MCP result semantics recorded in §13.1. The CLI flag for `DescribeOptions::tool_namespaces` (W2-9) is `--tool-namespaces ns1,ns2`, for `--describe=tools-*` and `frap mcp`. `frap-cli/src/main.rs` uses `#![deny(unsafe_code)]` instead of `forbid` so the linkme registration can take a local `allow` (RFC 0001 F-LINK-1). The `frap-schema` ready-event description now names serve too (integration fix, `protocol.snap` updated). | §13.1, §13.4, §14 |
| W4-1 | WP 1.20 object ops (sign-off) | `frap_op::pipeline` (WP 1.12 file) gains `pub const MACRO_STEP_FORCED` and `pub fn macro_step_forced(macro, step)`: step properties a macro definition forces (`OBJECT_OT_duplicate_move_linked` forces `OBJECT_OT_duplicate.linked=True`, as `RNA_boolean_set(otmacro->ptr, "linked", true)`; a user's `linked=False` is ignored, verified on 5.2.4); `execute()` applies them. `convert_kwargs` without a context accepts any string/identifier for a dynamic enum whose static item list is empty (`open_dynamic`); the operator re-checks it in exec (`object.collection_link`: `enum "X" not found in (...)`). `frap-session/src/session/exec.rs` (WP 1.14): the op statement calls `frap_op::pipeline::execute(def, cx, args)` instead of `(def.exec)(cx, args)` (identical for `OpKind::Normal`; macros now run their steps). Shared helpers live as inline `common` submodules of owned files (`select_by_type`, `move_to_collection`, `material_slot_add`, `camera_add`); `join`'s mesh merge is `ops/join/mesh.rs`. `frap-ops-data/src/ops/_dir.rs` deleted. At integration `object.duplicate_move`/`duplicate_move_linked` were switched from `Partial` to `Full` (transform.translate landed with WP 1.21), their `--describe` snapshots regenerated, `tests/cli/m1/object_ops/duplicate_move.*` rewritten (locations and shared meshes checked against 5.2.4) and golden `object/duplicate/move_macros` added. Deviations D-44, D-45, D-46, D-51. **Follow-ups:** a `#[operator(macro_forced = [...])]` attribute in `frap-macros`/`OperatorDef` should replace the hard-coded `MACRO_STEP_FORCED` table (WP 1.12); the session should convert arguments with the context (`pipeline::prepare`) so dynamic enums are checked before poll and `open_dynamic` can go (WP 1.14/1.12). | §10.1, §10.2 |
| W4-2 | WP 1.21 transforms, parenting, origin, apply, shading, snapping | `frap-ops-transform` and `frap-ops-object` depend on `libm` (rule 2.2.12). `transform.translate/rotate/resize` are registered with a hand-written `OperatorDef` in `OPERATORS` (not `#[operator]`), because `TransformModeHandler::apply` takes the raw `OpArgs`; a unit test per file checks the spec pointer. `frap-ops-transform/src/ops/_dir.rs` deleted. WP 1.21 also owns `tests/golden/fixtures/object/{transform,parent,origin,apply,shade,snap}/**` and `tests/cli/m1/object_ops/transform_parent_apply.*`. The axis-angle gimbal orientation is derived from Blender's results (every axis/angle probed, incl. the degenerate `(1,1,1)` axis). Deviation D-47. **Open:** `ObjectModifiers.active` has no RNA get hook (the `parent_set` golden checks `is_active` in unit tests only); `deformed_by` ignores virtual modifiers (M6). | §10, §22.3 |
| W4-3 | WP 1.23 IO operators and `.frap` mapping | `frap-op`: `OpError::UnsupportedFormat(String)` (`RuntimeError`, code `IO_UNSUPPORTED_FORMAT`, message `Error: …`; the enum is `non_exhaustive`), `OpCtx::{win_mut, prefs_mut}`. `frap-scene::Host` gains the default method `prefs_mut(&mut self) -> Option<&mut Preferences> { None }`; `frap-session`'s `HostImpl` implements it (sets `context_changed`). `frap-project` depends on `glam`; `scene_map::Saved` gains `header_only: Vec<String>`. §15 layout: linked IDs are written under `"$linked"` (not `"linked"`, which could collide with a local ID named `linked`); orphan paths are reported as `meshes["X"]`. `frap-ops-io-tests` dev-depends on `frap-nodes`, `frap-spec-ops-io`, `frap-io-gltf`, `frap-golden`, `glam`. `crates/frap-session/tests/m0_session.rs` (WP 1.14): `GLTF_SEPARATE` export now succeeds and writes `y.gltf` + `y.bin`. `export_scene.gltf` accepts all of Blender's props (the §2 gap is closed). `wm.revert_mainfile`'s "file saved" requirement is a `--describe` note until a `PollReq` exists (§2 gap). Golden cases cannot import committed Blender-made files (they run in a scratch directory): a WP 1.8 harness feature; a Rust test in `frap-ops-io-tests` covers it meanwhile. Deviations D-08 (widened), D-48, D-49, D-50. | §10, §15, §17.2 |
| W4-4 | WP 1.24 render operator | New test-only workspace crate `crates/frap-ops-render-tests` (layer 12 in `xtask/src/check_deps.rs`, rule 2.2.11: `frap-ops-render` cannot dev-depend on `frap-session`). `.github/workflows/gpu.yml` runs `cargo test --locked -p frap-ops-render-tests -- --nocapture` with `FRAP_WGPU_BACKEND`, so render goldens gate on Metal, lavapipe and WARP. Render golden mechanism: the shared harness (WP 1.8) cannot compare frap-blessed images nor run render lines without a GPU, so render cases mark their render statements with `#@render` (a comment for the generic suite and `golden_runner.py`) and name the image with `# render-golden: image=<path>`; `frap-ops-render-tests` runs them and compares against `tests/golden/fixtures/render/m1/<case>.frap.png` (gating) and `<case>.blender.png` (reported). Bless with `FRAP_RENDER_BLESS=1 cargo test -p frap-ops-render-tests --test render_goldens`. `frap-ops-render/src/ops/_dir.rs` deleted; `output.rs` has submodules `output/{template,format}.rs`. | §19, §20.5 |
| W4-5 | WP 1.26 goldens and exit transcript | Ownership extended to `tests/cli/m1/vertical_slice*` (trycmd cannot feed stdin from a `.trycmd`, so the raw-stdin and serve modes are `.toml` cases; §22.3 updated). The temporary gate (`console,ignore` fences, `status = "skipped"`) was removed at integration; the exit transcript passes in raw, stdin and serve modes on macOS. Blender-compared goldens for `IO_ERROR`, `OP_EXEC_ERROR` (`Cannot render, no camera`) and the W4 operators' `poll()` are pinned by the transcript's `ops_errors.py`/`ops_polls.py` (checked against 5.2.4 by the WP) and can now move to `tests/golden/cases/{errors,poll}`. **Open (WP 1.8):** `FRAP_GOLDEN_FILTER` has no brace alternation, so `{a,b}/**` matches zero cases and the test passes vacuously; the harness should support it or fail on an empty match. | §20, §22.3 |
| W4-6 | Shared target directory and golden bless | `cargo xtask golden bless` runs `cargo run -p frap-golden`, whose repo root is fixed at compile time (`CARGO_MANIFEST_DIR`); with one `CARGO_TARGET_DIR` for every worktree, a binary built in another worktree can bless the wrong tree. WPs blessed with their worktree-specific `deps/frap_golden-<hash>` binary; the integrator re-ran the gate from the main tree after the worktrees were removed (W2-15). | §22.1 |
| W4-7 | Integration fixes | Merge order 1.21, 1.20, 1.24, 1.23, 1.26 (`--no-ff`, no textual conflicts; `Cargo.lock` merged cleanly). Fixes: `tests/golden/fixtures/gltf/m1/import_separate.blender.json` re-blessed (WP 1.23 lengthened the case header after blessing, so line numbers drifted); the `op__import_scene.gltf`, `op__wm.read_factory_settings`, `op__wm.revert_mainfile` and both `op__object.duplicate_move*` snapshots re-blessed; the 15 stale `harness/*` lines of `crates/frap-golden/rna_fallbacks.txt` deleted (their bindings landed in W2/W3); `tools/blender/golden_runner.py::NORMALISE` gains D-44 to D-51; `docs/OPERATORS.md` ticks the 32 W4 operators (registered, goldens green, `--describe` snapshots present). | §20, §22.1 |
| S1-1 | M1 stabilization round 1 (exit-verifier findings) | `frap-op`: `convert_args` returns `(OpArgs, Vec<Report>, Vec<Hint>)` and the session copies the hints into operator responses (`ANGLE_LIKELY_DEGREES`, §13.2). `frap-session`: Python-only collection members (`Object.children`, `children_recursive`, `users_collection`, `users_scene`) resolve as property places that dereference to lists (no `Target::Coll`); a multi-dimensional `bpy_prop_array` (`bound_box`) reads one row with `[i]`; `Startup::File` builds the factory state then runs `wm.open_mainfile`; `Scene::UNCLAMPED` is honoured by assignments; assignment errors on a collection API struct (`view_layer.objects.active`) use the `BPy_PropertyRNA - Attribute (setattr):` prefix; `bpy.context.{visible,selectable,editable}_objects` are routed. `frap-rna::coerce`: a standard node socket's scalar `default_value` has an unbounded hard range (`rna_NodeSocketStandard_{float,int}_range`). `Object.dimensions`/`bound_box` are `=> hook` bindings whose hooks live in `frap-ops-data/src/hooks/` (the only layer with both `frap-eval` and the registry; frap-ops-data gains `frap-eval` and `glam`): reads evaluate with a throwaway `EvalCache`, writes use the session's evaluator. `frap-mesh` gains `Mesh::{domain_size, unique_attribute_name, add_attribute, remove_attribute, uv_map_count, add_uv_map, update, transform, clear_geometry, validate_report}`, `AttributeError`, `MAX_UV_MAPS`, `default_column` and `attribute_srna`; new RNA functions `Mesh.{update, validate, calc_loop_triangles, transform, clear_geometry}`, `AttributeGroupMesh.{new, remove}`, `UVLoopLayers.new`. Moved to M2 (OPERATORS.md, deviations.md §2): `frap.describe` (the manifest lives in `frap-schema`, above the ops crates), `bpy.context.preferences`/`space_data`, `Mesh.{set_sharp_from_angle, flip_normals, split_faces}`, `uv_layers.remove`, `vertex_colors.new`. | §9.2, §9.5, §10.2, §13.1, §13.2 |

## Decision log (M2 W0, WP 2.0: contract delta and stubs)

WP 2.0 started M2 on branch `m2-edit-mode` from the M1 stabilization head. It first closed the M1 leftovers the M1 exit review and CI found (M2W0-1..4), then applied the M2 contract delta. Every change below is additive unless stated otherwise.

| # | Topic | Decision | Where |
|---|---|---|---|
| M2W0-1 | M1 CI failures (PR #1, run 44176195828/44176195702) | Windows clippy: a unix-only test module of `frap-io-gltf/src/import/images.rs` is `cfg(all(test, unix))`. `gpu.yml`: Git Bash's GNU `tar` cannot read the WARP `.nupkg` (a zip); Windows' own `C:\Windows\System32\tar.exe` (bsdtar) extracts it. `cargo deny`: RUSTSEC-2026-0542 (`humantime`, unmaintained because `chrono` is; no safe upgrade) is ignored with a reason: it is dev-only (`trycmd` → `humantime-serde`). The Linux test job failed the M1 exit transcript because it renders and the job had no adapter: `ci.yml`'s test job now installs lavapipe (Linux) and WARP (Windows) like `gpu.yml`, and `frap-cli` turns `wgpu_hal` probing logs off by default (`warn,wgpu_hal=off`; `--log-level`/`FRAP_LOG` override), so a missing adapter no longer prints Vulkan loader errors. These fixes need CI re-validation on the three OSes (the integrator does not push). | `.github/workflows/*`, `deny.toml`, §20.5 |
| M2W0-2 | `--max-elements` pre-check, u32 overflow | `frap_math::budget` (L0, thread-local): `Budget { timeout, max_elements }`, `scope`, `check`/`check_now`, `reserve_corners(corners, largest_domain)`, `ElementsExceeded`, `MAX_MESH_ELEMENTS = i32::MAX`. Every primitive struct has `estimate() -> Option<Estimate>` (checked `u64` arithmetic; exact for all but the cone, an upper bound there; tested against the generated meshes); every `mesh.primitive_*_add` calls `common::reserve` before building and answers `LIMIT_EXCEEDED` (`…: the statement would create N face corners (limit M)`, or `…more than a Blender mesh can index`). `create_grid` computes in `usize`; the generator pools refuse more than `u32` elements with a clear panic message (only reachable through the Rust API without a pre-check). The post-statement check counts a mesh both added and modified once (it was counted twice). | §12, `frap-math/src/budget.rs`, `frap-mesh/src/primitives/` |
| M2W0-3 | `--stmt-timeout` | A real deadline with cooperative cancellation: `budget::check()` reads the clock every 1024 calls and unwinds with `resume_unwind(Interrupted)` (no panic hook output); the session opens the budget scope per statement (`exec`, `invoke`, `transact`), maps the payload to `RuntimeError` code `TIMEOUT` (`Error: frap: limit exceeded (stmt_timeout)`, `detail.stmt_timeout_ms`, rollback), and also fails a statement that finished after its deadline. Check points: primitive builder, polyfill ear loop, face/corner normals, looptris, corner split, glTF extraction and per-object gather, render-scene per object, before/after each GPU render (never inside wgpu error scopes). BMesh ops must call `check()` in long loops (lib docs). New code registered by frap-session; D-52. | §12, §13.4, deviations D-52 |
| M2W0-4 | mathutils on RNA values, mesh layer hooks | `resolve.rs`: `FuncTarget::Math { recv, prop, name }` binds a mathutils method to a value read from RNA (`ob.matrix_world.to_euler('ZYX')`, `.to_quaternion()`, `.decompose()[0]`, `location.normalized()`, `.angle(v)`, `.lerp(v, f)`, …) and calls `session::eval::method`; read-only attributes `Matrix.translation/is_negative/median_scale`, `Quaternion.angle/axis/magnitude`, Vector swizzles (`.xy`, `.zyx`, Blender's `Vector swizzle: specified axis not present`). `Vector.length` follows `dot_vn_vn` (f32 products, double sum, last component first), `Vector.angle` rounds the double `acos` to float, `median_scale` is `mat3_to_scale`: all verified bit-exact on 5.2.4. New `eval::method` members: `Vector.to_2d/angle/lerp`, `Quaternion.to_axis_angle`, `Matrix.inverted_safe/decompose`. RNA hooks in `frap-ops-data/src/hooks/`: `UVLoopLayers.active/active_index`, `AttributeGroupMesh.active_color/active_color_index/active_color_name/default_color_name/render_color_index` (names are stored as given, as Blender does). `attributes.active/active_index` stay `NOT_IMPLEMENTED` (§2 gap, WP 2.9). Goldens `rna/mathutils_members`, `rna/mesh_layer_active`; D-53. | §9.2, deviations D-53 |
| M2W0-5 | Edit-mode wiring | W0 wired every non-owned call site WP 2.2 needs to files WP 2.2 owns, with functional stubs that keep M1 behaviour: `frap-session/src/edit/mod.rs` (`mesh_view`, `check_write` before every property/collection write, `check_func` before RNA functions, `edit_mesh_block` in every response), `frap_op::helpers::edit_merge::merge_into_edit_mesh` (called by `add_primitive_or_merge` in Edit Mode), `frap_op::helpers::edit_mode::reenter_after_load` (called by `wm.open_mainfile`'s `replace_state`; `scene_map/read.rs` now loads `mode: EDIT` as `ObjectMode::Edit`). `OpCtx::bmesh_mut` is implemented (logs the mesh as modified without a `gen` bump, bumps `bm_gen`, resets the view, `Arc::make_mut`). `bpy.context.objects_in_mode[_unique_data]` use `Context::objects_in_mode*` (multi-object). | §6.3, §8, §10.1 |
| M2W0-6 | `frap-bmesh` layout | `src/ops/` is a `pub` `automod` directory (no registration, so W0 stubs every listed op: input struct = Blender's input slots with their defaults, from [MESH App. C], `#[non_exhaustive]` output struct, `todo!()` body). `ops/delete.rs` and `ops/duplicate.rs` move to WP 2.1 (W1): extrude, split, separate, spin and solidify build on them, which would otherwise be same-wave impl dependencies. Remaining same-wave uses are *(soft)*: `solidify`/`spin` → `extrude_face_region` (2.5), `symmetrize` → `bisect_plane`/`weld_verts`/`reverse_faces` (2.8/2.7/2.9), `bridge_loops` cuts → `subdivide_edgering` (2.6). `bevel` is `ops/bevel.rs` + `ops/bevel/**` (automod skips subdirectories). `BMesh::delete_context` (bmesh_delete.cc) is a kernel function. | §6.2, §22.4 |
| M2W0-7 | Workbench completion and contact sheet | `RSettings::workbench: RWorkbenchShading` (every `scene.display`/`display.shading` value the complete Workbench reads; filled by `build/workbench.rs::shading`), `RObject::display_type`, `SheetLayout`/`SheetTile` and `RenderService::composite_sheet` (default body returns `Unsupported`; `frap-ops-render` reaches the GPU and the compositor only through the trait). WP 2.3 needs the uber-shader, engine, service, the render-scene builder and the render golden runner, which no other M2 WP touches: they are reassigned to WP 2.3 for M2 (§22.4 row). `frap-render/src/workbench/{matcap,xray,wire,outline,shadow,cavity}.rs` and `composite/mod.rs` are doc stubs. | §19.2, §19.3, §19.6 |
| M2W0-8 | `frap.*` operator specs, `frap.describe` | One hand-written spec file per operator, owned by the implementing WP: `spec/frap/ops/frap.{select_elements,select_by_normal,scene_summary,render_contact_sheet}.json` (props: `select_elements(type, indices, action=SET\|ADD\|REMOVE, object)`, `select_by_normal(direction, threshold, orientation, extend)`, `scene_summary(objects, evaluated)`, `render_contact_sheet` as §19.6, `frame` with is-set semantics); `frap.json` keeps `describe`. `tools/blender/emulations.py::select_elements` follows the new signature. `frap.describe`: `frap_op::DESCRIBE_PROVIDERS` (`DescribeProvider { describe: fn(&str) -> Result<Json, FrapError> }`); `frap-schema/src/provider.rs` registers it (stub until WP 2.13, which owns it and `frap-ops-frap/src/ops/describe.rs`). | §14, §22.4 |
| M2W0-9 | OPERATORS.md M2 rows without a WP (rule 1) | `object.material_slot_select/deselect` → 2.4; `object.material_slot_assign`, `Mesh.set_sharp_from_angle/flip_normals/split_faces`, `uv_layers.remove`, `vertex_colors.new` → 2.9; `transform.transform` → 2.12 (`ops/transform.rs`); Context `preferences`/`space_data` → 2.3 (`resolve/context/ui.rs`, dispatched from `context.rs`); `frap.describe` → 2.13. `TransformOp` is not extended: `transform.mirror`/`push_pull` in Object Mode stay outside M2 (`OP_NOT_IMPLEMENTED` there; WP 2.12 may add variants additively, without changing `object/`). | OPERATORS.md, §22.4 |
| M2W0-10 | Dependencies | `frap-bmesh`: frap-geom, frap-math, bitflags, smallvec, smol_str, rustc-hash, libm, automod (+ proptest/approx/serde_json/insta dev). `frap-ops-mesh`: frap-geom, libm, rustc-hash, smallvec, bitflags. `frap-ops-frap`: frap-bmesh, frap-math, frap-eval, glam, libm, smallvec. `frap-ops-object`: frap-bmesh. `frap-ops-transform`: frap-bmesh, frap-mesh, frap-geom, rustc-hash, smallvec. `frap-ops-render`: frap-eval, frap-mesh, frap-math, glam, libm. `frap-render`: frap-math, smol_str. `frap-session`: frap-bmesh, libm. `frap-ops-data`: libm. `frap-schema`: linkme. `frap-ops-all` dev: frap-math. `check-deps` passes. | §3.2, §3.5 |
| M2W0-11 | Codegen and contract pins | `spec/current_milestone` = 2; `cargo xtask codegen` generates typed props for every M2 operator (new `frap-spec-ops-mesh/src/generated/geometry.rs`, `frap-spec-ops-frap/src/generated/frap.rs`). `crates/frap-ops-all/tests/contracts.rs` pins the BMesh API, the op stub form, `DESCRIBE_PROVIDERS`, the edit helpers, `composite_sheet`, `RWorkbenchShading` and `frap_math::budget`. | §4.2, §3.4 |

## Decision log (M2 W1 integration)

The three W1 packages (2.1 BMesh core, 2.2 Edit-mode integration, 2.3 Workbench completion and contact sheet) were merged into `m2-edit-mode` with `git merge --no-ff` in the order 2.1, 2.2, 2.3. WP 2.2 had already merged `wp/2.1` (bd3d259) to build on its own. There were no textual conflicts. Every change a WP made outside its §22.4 paths, and every addition to a frozen contract, is recorded here; additive unless stated otherwise. Behaviour that differs from Blender is in `deviations.md` (D-54 to D-63, §2 gaps updated).

| # | Topic | Decision | Where |
|---|---|---|---|
| M2W1-1 | WP 2.1 BMesh types (additive) | `BMLoop.flag: ElemFlag` (Blender loops carry UV-select flags); `ElemFlag::{SELECT_UV = 64, TAG_ALT = 128}` and the alias `SELECT_UV_EDGE` (= `SEAM`); `BMesh` gains `index: ElemIndex`, `oflags: ToolFlags` and `BMesh::with_alloc_size`; `BMesh::new()` uses Blender's pool chunk sizes; new `OpTypeFlag`, `ElemIndex`, `ToolFlags` (not re-exported at the crate root: `frap_bmesh::types::…`); `From<VertId/EdgeId/FaceId> for BMElem`; `vert/edge/lp/face(_mut)` and `oflag_*` helpers. `Pool` gains pub `pchunk`/`chunks`, `with_chunks`, `next_slot`, `ids`, `contains`, `ChunkSizes`. `CustomData` gains `deform`, `uv_active`/`uv_render`, `add_layer` (type-sorted), `remove_layer`, `get`, `float2_names`, `cd_type` and the `*_attrs_copy`/`data_interp_*` helpers. | §6.2, `frap-bmesh/src/{types,pool,customdata}.rs` |
| M2W1-2 | WP 2.1 behaviour vs the frozen doc strings | `select_set` follows `BM_elem_select_set` (deselecting flushes down according to the select mode); `face_exists` matches either winding (`BM_face_exists`); `vert_separate` returns the original vertex followed by the new ones, most recent first. `select_history_store` keeps the contract's move-to-end behaviour; Blender's store-if-absent is the new `select_history_store_if_absent`. The doc strings were corrected by the WP. | §6.2 |
| M2W1-3 | WP 2.1 public API beyond the contract | `edit_mesh_make`, `edge_split(_n)`, `face_split`, `faces_join`, `face_create_ngon`, `face_kill_loose`, `join_vert_kill_edge`, `edge_separate`, `loop_reverse`, `bmo_exec`, `bmesh_edit_end`, `delete_oflag_context` and `kernel::DeleteContext`, the `*_ex` walkers with `WalkOpts`/`LoopDelimit`, hide functions, select-flush variants, `validate_strict`, query and normals helpers; `DuplicateOut` gains `geom_orig` and the vert/edge/face/boundary/isovert maps. New files inside `crates/frap-bmesh`: `src/query/geom.rs` (rule 22.1 submodule), `tests/{core_vs_blender,roundtrip_primitives,prop_kernel,live_blender}.rs`, `tests/common/mod.rs`, `tests/blender/dump_core.py`, `tests/fixtures/core.blender.json` (1.8 MB). | §6.2, §22.1 |
| M2W1-4 | WP 2.2 non-owned edits | `frap-session/src/resolve/access.rs` (WP 1.14): `with_struct` sends RNA paths rooted at a mesh in Edit Mode to `edit::edit_view`. `frap-session/src/session.rs`: one line, `edit_mesh_block(&host, verbosity == Full)`; new `Session::edit_mesh_block(full)`. `frap-golden/src/runner.rs` (WP 1.8): the dumper fills `selection`, `active`, `select_history` of meshes in Edit Mode from `Session::edit_mesh_block(true)` (new `edit_state`); the run-time rejection of `mesh:` compares in `EDIT_MESH` is removed; `Dumper.edit_mesh` removed. `tools/blender/dump_state.py`: edit meshes are dumped from `me.copy()` (D-05 golden column) with the new `dump_edit_state`; `tests/golden/README.md` describes it. `frap-mesh` (WP 1.4): new `primitives/trace.rs` (`Build`, `ConeTopology`, `take_last`, recorded by `Bm::to_mesh`), `Bm` fields `uvs_follow_face_order`/`cone` (object-mode output unchanged); `rna/mod.rs`: `Mesh.total_{vert,edge,face}_sel` are `Field::NeedsHook`, served by `frap-session/src/edit/hooks.rs` (with the new set hook `ToolSettings.mesh_select_mode`). | §6.3, §13.2, §20.2 |
| M2W1-5 | `edit_mesh` response block | Besides the documented counts, `selected` and `active`, the block carries `select_mode`, `select_history` (capped at 8 entries unless `--verbosity full`) and `select_history_len`. | §13.2 |
| M2W1-6 | WP 2.2 spec and tests | `spec/frap/ops/frap.select_elements.json`: the `indices` minimum is `INT_MIN`, so a negative index raises `IndexError` like the Blender emulation (generated `frap.rs` regenerated). `frap-ops-frap/src/ops/_dir.rs` deleted. New describe snapshots `op__{frap.select_elements,mesh.select_all,mesh.select_mode,object.editmode_toggle}`. M1 tests updated for the new behaviour: `frap-op/tests/helpers.rs` (a merge in Edit Mode reaches `edit_merge`), `frap-ops-io-tests/tests/edit_mode_load.rs` (Edit Mode is re-entered on load), `tests/cli/m1/object_prereq/delete_and_modes.{stdout,toml}` (`mode_set EDIT` works). | §14, §20 |
| M2W1-7 | WP 2.3 render pipeline (non-owned) | `frap-ops-render/Cargo.toml` + `frap-spec-ops-frap` (missed by W0). `frap-ops-render/src/output/format.rs` (WP 1.24): `file_pixels` puts straight-alpha films over black for non-RGBA outputs and premultiplies float RGBA (`IMB_alpha_under_color_float`). `frap-render/tests/workbench.rs` (WP 1.7): half-float tolerance (2e-3), Blackman-Harris log-space coverage test, 32-sample precision test with Blender's alpha 0.99316406. Films of opaque pixels carry Blender's alpha (0.998046875 at 8 samples). Main depth target `Depth24PlusStencil8` (D-59). `render.render` warns about unrendered cavity/DOF through `frap__render_contact_sheet::workbench_left_out` (now `pub(crate)`). M1 render fixtures `tests/golden/fixtures/render/m1/*.frap.png` re-blessed (renderer changed; `.blender.png` unchanged); new `render/m2/**`. `SheetLayout::background` stays documented as scene-linear, but the op passes display-referred tiles and background (the compositor works in the space it is given). | §19.2, §19.3, §19.6 |
| M2W1-8 | WP 2.3 `studio_light` routing and codegen | `xtask/src/codegen/gen_plain.rs` (WP 1.1): `PlainSpec` gains `hooked` (stored field whose RNA get/set go through hooks) and `extra` (non-RNA `SmolStr` fields). `View3DShading`: `studio_light` hooked; new DNA fields `matcap`, `lookdev_light` (`.frap` `scene.json` gains them with `serde(default)`). `spec/overlay/dynamic_enums.toml`: `View3DShading.studio_light` items = studio rigs + 8 world HDRIs + 27 matcaps with Blender's `StudioLight.index` (generated `enums.rs` regenerated). New `frap-scene/src/hooks/view3_d_shading_studio_light.rs` (get/set hooks ported from `rna_space.cc`, manual `HookDecl`). | §4.2, §5.6, §19.3 |
| M2W1-9 | WP 2.3 context and pipeline (non-owned) | `frap-session/src/resolve/{access,write,context}.rs` (WP 1.14): `Root::Context` pointers with a path walk from / write through `context::ui`; `mod ui` is `pub(crate)`; `window` is routed to `ui` (was `NOT_IMPLEMENTED`). `frap-op/src/def/pipeline.rs` (WP 1.12): an explicit `None` for an INT/FLOAT prop of a `frap.*` operator counts as unset (§19.6 updated). `tests/cli/m1/vertical_slice.{in/error_codes.py,trycmd}`: the `NOT_IMPLEMENTED` context example is now `bpy.context.active_operator` (space_data resolves). New snapshot `op__frap.render_contact_sheet`. | §9, §10.2, §19.6 |
| M2W1-10 | Integration edits | Stale `--describe` support/notes strings refreshed (`frap-ops-mesh` primitive `common::{NOTES,PARTIAL}` and `primitive_torus_add`, `frap-ops-object` `mode_set`, `frap-ops-io` `wm.open_mainfile`) and their describe snapshots re-blessed; `tools/blender/golden_runner.py::NORMALISE` gains D-54 to D-63; `docs/deviations.md` closes the WP 2.2/2.3 gaps of §2 and adds the remaining ones; `docs/OPERATORS.md` ticks `mesh.select_all`, `mesh.select_mode`, `object.editmode_toggle`, `frap.select_elements`, `frap.render_contact_sheet`, Context and Preferences (registered, goldens green, snapshots present). | §20, §22.1 |
| M2W1-11 | Open follow-ups (not blocking) | `frap-scene` `ViewCache` is a `OnceLock` keyed on `bm_gen` only, so the edit-mode view is re-grafted on every read after a non-geometry write until the BMesh changes (perf; needs a key on `(bm_gen, original generation)`). `export_scene.gltf(export_format='GLTF_EMBEDDED')` is refused even with `allow_embedded_format=True` (dynamic enum checked against static items in `frap-session` `convert_args`, WP 1.23 area). `frap_op::helpers::view_rotation` derives `align='VIEW'` from the stale `view_matrix` (0.76° roll) instead of `viewquat` for users other than `camera_add` and the primitives. `view_size` above 4096 is only an INFO clamp report; sheets above 4096 x 4096 are a `RuntimeError`. | — |

## Decision log (M2 W2 integration)

The nine W2 packages (2.4 to 2.12) were merged into `m2-edit-mode` with `git merge --no-ff` in the order 2.4, 2.6, 2.7, 2.9, 2.8, 2.5, 2.10, 2.11, 2.12 (siblings one at a time: an octopus merge of 2.5, 2.7, 2.8 and 2.9 conflicts on `bisect_plane.rs`, sequential merges do not). Two textual conflicts, both resolved by keeping both sides (M2W2-1). Every change a WP made outside its §22.4 paths, and every addition to a frozen contract, is recorded here; additive unless stated otherwise. Behaviour that differs from Blender is in `deviations.md` (D-64 to D-84, D-51 extended, §2 gaps updated).

| # | Topic | Decision | Where |
|---|---|---|---|
| M2W2-1 | Merge conflicts | `frap-op/src/def/pipeline.rs` `MACRO_STEP_FORCED`: WP 2.6's `MESH_OT_offset_edge_loops_slide` / `TRANSFORM_OT_edge_slide` / `single_side=true` entry and WP 2.5's 16 extrude-macro entries (`use_proportional_edit=False`, `mirror=False` on every extrude macro's transform step; `extrude_manifold` also `use_dissolve_ortho_edges=True` and `use_automerge_and_split=True`, from `ED_operatormacros_mesh`) are both kept; the table is `#[rustfmt::skip]`, one entry per line. `frap-schema/tests/snapshots.rs::manifest_selected_operators` (WPs 2.5, 2.10 and 2.11 each replaced an example that became implemented): the examples are now `uv.smart_project`, `armature.extrude_move`, `object.modifier_add`, `mesh.decimate`, with their `manifest__op__*.snap`; the old `mesh.{bevel,extrude_region_move,spin}` manifest snapshots are gone. | §10.2, §20 |
| M2W2-2 | `frap-op` contract additions (WPs 2.6, 2.11) | Accepted: `OpCtx::is_macro_step()` (set by `run_def` when `merge_result` is true; `mesh.loopcut` needs Blender's `op->opm != nullptr` switch to edge select mode). `OpOutput.hints: Vec<pipeline::Hint>`, `OpCtx::hint(code, message)`, and `run_def` passes sub-operator hints up; `frap-session/src/session/exec.rs` lists the argument-conversion hints, then the operator's own (`NO_OP_DEFAULT` of `mesh.bevel`). | §10.1, §10.2, §13.2 |
| M2W2-3 | `frap-bmesh` additive API (all W2 BMesh WPs) | Accepted. Output slots of Blender's operators added to the frozen `#[non_exhaustive]` `*Out` stubs: `ExtrudeVertIndivOut.{verts,edges}`, `Inset{Individual,Region}Out.faces`, `ExtrudeDiscreteFacesOut.faces` (2.5); `subdivide_edges` `geom_inner`/`geom_split`, `subdivide_edgering.faces`, `offset_edgeloops.edges` (2.6); `Dissolve{Edges,Faces,Limit}Out.region`, `FindDoublesOut.targetmap` (2.7); `faces`/`edges`/`geom_cut`/`geom_interior`/`geom_unused`/`geom_holes` of the 11 create/fill ops (2.8); `TriangulateOut{edges,faces,face_map,face_map_double}`, `SplitEdgesOut.edges`, `SplitOut{boundary_map,isovert_map}`, `PokeOut{verts,faces}`, `SpinOut.geom_last`, `WireframeOut.faces`, `JoinTrianglesOut.faces` (2.10); `BevelOut{faces,edges,verts}` (2.11). `DissolveEdgesIn` has a manual `Default` with `angle_threshold = PI` (`bmo_dissolve_edges_init`; the stub derived 0). `TYPE_FLAG` consts per op file; new public functions and submodules of owned op files (`subdivide_edges::{Falloff, CornerType, Pattern, PATTERN_TABLE, ESubdivide, SubdivSelect, mesh_esubdivide}`, `unsubdivide::decimate_unsubdivide_ex`, `pointmerge::{pointmerge_facedata, average_vert_facedata}`, `collapse::collapse_uvs`, `find_doubles::remove_doubles`, `contextual_create::{bli, construct, edgenet, edgeloop_fill, attribute_fill, split_edgenet, connect_islands, kdopbvh}`, `bridge_loops::edgeloop`, `beautify_fill::heap`, `triangle_fill::scanfill`, `connect_verts::{concave, nonplanar}`, `rotate_edges::{edge_rotate, edge_rotate_check, edge_face_pair}`, `bisect_plane::mesh_bisect_plane`, `convex_hull::bullet` (the earlier public `convex_hull::hull3d` is removed, it had no users), `triangulate::{QuadMethod, NgonMethod, FaceTriangulation, face_triangulate, mesh_triangulate, beautify}`, `split_edges::{vert_separate_hflag, mesh_edgesplit}`, `wireframe::mesh_wireframe`, `symmetrize::direction_value`, `region_extend::TYPE_FLAG`). `BMEdgeLoopStore` is ported twice (`subdivide_edgering::edgeloop`, `pub(crate)`, WP 2.6, and `bridge_loops::edgeloop`, WP 2.8, also used by `grid_fill`): kept as is, deduplication is a follow-up (M2W2-13). | §6.2 |
| M2W2-4 | WP 2.9 non-owned edits (accepted and re-homed) | `frap-mesh/src/mesh.rs`: `Mesh::attributes_active: Option<SmolStr>` (the active attribute by name; Blender stores an index, D-72) and `rna/attribute.rs` `rename()` keeps it. `frap-project` `scene_map` `{read,write}.rs`: optional mesh key `attributes_active`. `frap-session/src/edit/mod.rs` `graft_non_geometry` copies it. `frap-ops-data`: new hooks `hooks/attribute_group_mesh_active{,_index}.rs` (`mesh.attributes.active`/`active_index`) and `funcs/attribute_group_mesh_remove.rs` (keeps the active index); from now on owned by WP 2.9's successors (attributes). `frap-bmesh/src/types.rs` (WP 2.1): `BMesh::lnor_spacearr: LnorSpaceArr { dirty_all, data }` with `invalidate_all()`/`clear()`; `kernel.rs` element allocation/kill and `bmesh_edit_end` (`NORMALS_CALC`, `INVALIDATE_CLNOR_ALL`) mark it dirty (port of `bmesh_core.cc`/`bmesh_mesh.cc`); `OpTypeFlag::INVALIDATE_CLNOR_ALL` is no longer a no-op. Shared submodules of owned files: `geometry__attribute_add/{attrs,common,convert}.rs`, `flip_normals/clnor.rs`. | §6.1, §6.2, §15 |
| M2W2-5 | WP 2.4 selection | `tools/blender/emulations.py` (WP 1.8 file; its docstring assigns them): `select_by_normal` (clamps to ±FLT_MAX like the property, normalises in double precision, `ValueError` for NaN) and `scene_summary` emulations in `EMULATED`. 23 new `op__*` describe snapshots. Golden cases renamed (`edit/select/{sharp_flat,loose_sides,region_loop,multi_object}` split into `sharp_*`, `flat_*`, `face_sides_*`, `loose_*`, `region_to_loop*`, `loop_to_region*`, `multi_object_*`, `ring_multi*`, `loop_ring_multi_nodelimit`) and `edit/select/stages/` (59 prefix cases that pin intermediate states, since the harness does not record expression values). At integration `select_by_attribute::active_attribute_name()` reads `Mesh::attributes_active` (WP 2.9), new golden `edit/select/by_attribute_active` (BOOLEAN point and face attributes, FLOAT and corner-domain poll messages). `select_linked`'s last-delimit memory (`USE_LINKED_SELECT_DEFAULT_HACK`) is not implemented: it is process-global state, D-64. | §20, §22.4 |
| M2W2-6 | WP 2.5 extrude macros | WP 2.12 landed, so 6 of the 7 extrude macros switch from `Support::Partial` to full and their goldens `edit/extrude/macros/*.py` are enabled (describe snapshots re-blessed). `mesh.extrude_manifold` stays `Partial`: its forced `TRANSFORM_OT_translate.use_automerge_and_split=True` needs `EDBM_automerge_and_split`, which edit-mode transforms do not implement (OP_NOT_IMPLEMENTED, §2 gap); `macros/manifold.py.pending` stays blessed but disabled. 2.12 is recorded as a soft dependency of 2.5 in §22.4. New CLI transcripts `tests/cli/m2/extrude/` (2.5) and `tests/cli/m2/delete/` (2.7) are accepted ahead of WP 2.13, which owns `tests/cli/m2/**` in W3 (rule "each WP ships `tests/cli/<milestone>/<topic>/`"). Mirror-modifier clipping (`mirror_exclude`) has only a unit test until a golden can add the modifier (M4). | §22.4 |
| M2W2-7 | WP 2.6 subdivide and loops | Owns the subdirectory modules `frap-bmesh/src/ops/{subdivide_edges,subdivide_edgering,unsubdivide}/` (submodules of owned files). `mesh.loopcut` runs headless in Blender 5.2.4 under the golden runner's `VIEW_3D` override, so goldens call it directly (research doc §0.8/§4.5 corrected, `emulations.py` docstring updated, `OPERATORS.md` loopcut line). No goldens yet for `loopcut_slide`/`offset_edge_loops_slide` (follow-up now that `edge_slide` landed). | §20, [MESH App. B.7] |
| M2W2-8 | WP 2.8 create and fill | Soft dependencies on 2.7 (`dissolve_faces`, `weld_verts`), 2.9 (`reverse_faces`) and 2.10 (`split`, `triangulate`, `join_triangles`) recorded in §22.4. The five goldens that needed them (`convex_hull_defaults`, `edge_face_add_faces`, `bridge_dep`, `fill_grid_faces` at T1, `concave_split` at T2) were added to `tests/golden/cases/edit/create/` and blessed at integration. The `edbm::with_deps` guard (turned a sibling's `todo!()` panic into `NotImplementedError`) was removed: no M2 stub is left. `fill_grid` with faces selected now reads `SplitOut::boundary_map` instead of matching coordinates; that exposed a bug in WP 2.10's `split` port, which built the map before `BMO_mesh_delete_oflag_context(DEL_FACES)` marked what it deletes (every boundary key became its copy, so nothing was welded); `split.rs` now runs the marking passes first, as Blender builds the map in the delete's `prepare` callback. `BM_vert_splice_check_double_face` is ported only for vertex pairs of which one has no face (the only case `face_split_by_edges`' island connect reaches). New fixtures `tests/golden/fixtures/edit/create/*.blender.json`. | §6.2, §22.4 |
| M2W2-9 | WP 2.10 and 2.11 schema tests | `frap-schema/tests/{snapshots,acceptance}.rs` examples changed (M2W2-1); new describe snapshots of every implemented operator. WP 2.10 gave the stub `BisectPlaneOut` a `geom_cut` field with WP 2.8's exact text, so the merge was clean. The Suzanne two-bevel case is not a golden (6.4 MB fixture); the same path is covered by `edit/bevel/clamped_two_bevels` and a frap-bmesh unit test. | §20 |
| M2W2-10 | WP 2.12 edit-mode transforms | No non-owned source edits; new files only under `frap-ops-transform/src/{edit,ops}/**`, 29 fixtures under `tests/golden/fixtures/edit/transform/`, 11 `op__transform.*` snapshots. `edit::Request.orient_matrix_type` is `Option<String>` (is-set aware). `transform.transform` in Object Mode rebuilds `translate`/`rotate`/`resize` arguments and calls `crate::core::dispatch`. Code that moves vertices outside a BMesh operator calls `lnor_spacearr.invalidate_all()` (as `transform.cc`), M2W2-4. | §10, §14 |
| M2W2-11 | Integration edits | `--describe` support notes of `transform.translate/rotate/resize` (WP 1.21 files) now say "Object Mode and mesh Edit Mode (pose mode M6)" and their snapshots are re-blessed. `tools/blender/__pycache__/*.pyc` (committed by WP 2.4) removed; `.gitignore` ignores `__pycache__/` and `*.pyc`. `tools/blender/golden_runner.py::NORMALISE` gains D-64 to D-84 and the `fixtures` kind (D-67: fixtures are blessed on macOS). `docs/OPERATORS.md` ticks the 106 W2 operators that are registered with goldens green and snapshots present (`mesh.extrude_manifold` stays unticked, M2W2-6) and the WP 2.9 `Mesh` RNA functions. `docs/deviations.md` closes the `attributes.active` and `Mesh` function gaps of §2. | §20, §22.1 |
| M2W2-12 | Gate | `cargo build`/`test`/`clippy -D warnings`/`fmt --check`, `cargo xtask check-deps`, the CLI transcripts and the golden suite against Blender 5.2.4 (`cargo xtask golden live`) are green in the main tree. | §20.5 |
| M2W2-13 | Open follow-ups (not blocking) | Auto Merge in edit-mode transforms (`EDBM_automerge`, `EDBM_automerge_and_split`; enables `extrude_manifold`, bevel and symmetrize Auto Merge). Object Mode `orient_matrix`/`orient_matrix_type` resolution (`core/orient.rs`, WP 1.21; the edit path's `orient_props()` shows the fix). `select_linked` delimit memory needs a session-scoped operator-state slot if ever wanted (D-64). Goldens for `loopcut_slide` and `offset_edge_loops_slide`. A UV sphere entered into Edit Mode does not reach T1 against Blender (edge order; WP 2.1/2.2 area). `vert_connect_nonplanar` on meshes that already have duplicate edges differs from Blender. The golden harness ignores expression-line values (recording them would replace the `stages/` prefix cases). One `BMEdgeLoopStore` port instead of two (M2W2-3). `cargo fmt --check` does not see `automod` modules: `cargo xtask fmt [--check]` now formats every tracked hand-written `.rs` (generated files excluded) and CI gates on it (M2 stabilization round 3). The dangling active-attribute index model (D-72, would touch `frap-mesh`, hooks, `scene_map` and the edit graft). FLOAT4X4 attribute values are flat 16-float arrays in RNA (Blender `[4][4]`; RNA bindings owner). Enum arguments are checked before poll (D-78; W4-1 follow-up). A CLI transcript for `mesh.bevel()` / `offset=0` / `PERCENT` / `offset=float('nan')` hints (`NO_OP_DEFAULT`) goes to WP 2.13 (done, M2W3-2). Very large `loopcut` `number_cuts` is pre-estimated against `--max-elements` (M2 stabilization round 3). | — |
| M2W3-1 | WP 2.13 contract additions | Accepted (additive, on `#[non_exhaustive]` enums): `frap_rna::Value::Json(Box<serde_json::Value>)` carries free-form JSON results without an RNA type (the `frap.describe` fragment; `Value` has no map variant), and `frap-session/src/response.rs` `value_json` answers it unchanged; `frap_op::OpError::Frap(Box<FrapError>)` lets an operator raise its own exception and code (`DESCRIBE_TARGET`), `into_frap` passes it through (`OpError::Rna` still maps to `OP_EXEC_ERROR`). `frap-schema/src/provider.rs` gains `describe_target(&str)` and `link()` (for front-ends that link `frap-schema` only for `frap.describe`). `spec/frap/ops/frap.json` `describe.target` flags are `REQUIRED`, `NEVER_NULL`, `RUNTIME`; `PropFlags` does not model `RUNTIME`, so codegen drops it and the generated code / describe snapshot (`frap-schema/tests/snapshots/op__frap.describe.snap`, re-blessed) show `REQUIRED`, `NEVER_NULL` only. `frap-spec-ops-frap/src/generated/frap.rs` regenerated (`codegen --check` clean). | §14, §22.4 |
| M2W3-2 | WP 2.13 workflows and transcripts | 25 multi-op workflow goldens `tests/golden/cases/edit/workflows/*.py` (tables, chairs, mugs, stools, shelf, cabinet, vase, bowl, cushion) with Blender 5.2.4 fixtures; M2 CLI transcripts `tests/cli/m2/{bevel,contact_sheet,describe,edit_mode,workflows}/` (the `mesh.bevel` `NO_OP_DEFAULT` hint transcript of M2W2-13 is `bevel/no_op_hints`); edit-mode recipes in `docs/agents.md`. `.gitignore` ignores `tests/golden/cases/**/out/` (export outputs of cases run from their directory). `frap.describe` is ticked in OPERATORS.md (T0, no Blender counterpart). | §22.4 |
| M2W3-3 | Gate | `cargo build`/`test`/`clippy -D warnings`/`fmt --check`, `cargo xtask check-deps`, the CLI transcripts and the golden suite against Blender 5.2.4 (`cargo xtask golden live`) are green in the main tree after merging WP 2.13. | §20.5 |
| M2S1-1 | M2 stabilization round 1 (exit-verifier findings) | `export_scene.gltf` leaves the active object's mode first, as the add-on's `export.save` does (`bpy.ops.object.mode_set(mode='OBJECT')`): edit meshes (multi-object edit included) are written back before gathering and the session ends in Object Mode; new helper `frap_op::helpers::mode_set_object(cx) -> Result<bool, OpError>` (the `mode_set(mode='OBJECT')` path for operator bodies). The session runs §10.2 in Blender's order (positionals, poll, then keyword conversion with the context through `pipeline::convert`), closing the W4-1 follow-up; `OpError::Rna` of kind `Enum` maps to `OP_ARG_ENUM` (was `OP_EXEC_ERROR`); `mesh.merge` registers its `type` items (exact outside vertex select mode, D-78 narrowed to the remaining case). D-77 extended: `BEAUTY` diagonals of a near-regular n-gon flip on a 1-ulp upstream coordinate difference (Blender, given frap's coordinates, picks frap's diagonals: the port is exact). §22.4 Exit: the [MESH §5.8] P0 list is restricted to the edit-mode mesh operators (`uv.smart_project`, `uv.unwrap`, `uv.pack_islands` are M3 WP 3.8; `object.shade_auto_smooth` is M4). Golden `edit/workflows/export_from_edit_mode`, transcript `tests/cli/m2/edit_mode/export_from_edit`. | §10.2, §17.2, §22.4 |
| M2S2-1 | M2 stabilization round 2 (exit-verifier findings) | Workbench shadows apply to geometry only: the shadow resolve triangle sits on the far plane (`vs_full_far`) with `depth_compare: Greater`, so background pixels (depth 1, where depth-fail volumes whose back caps sit on the far plane leave non-zero counts) are never shadowed; render goldens may add `bg_max_delta_e=<x>` (every Blender background pixel compared pixel for pixel, `render/m2/shadows_background_{factory,objects,table}`). `--stmt-timeout`: new `frap_math::budget::check_n(work)` (clock read once per `CHECK_EVERY` units of work); `BMesh::face_loops` and the walks of `split_face_make_edge` are weighted cancellation points, so quadratic kernels on large n-gons (`loopcut`/`subdivide_edges`, `bevel`) time out promptly. `frap_mesh::primitives::trace::Build.circle` (`CircleTopology`): the Edit Mode merge replays `bmo_create_circle_exec` (`helpers::edit_merge::BuildPlan`, generalising the cone plan), so the NGON circle leaves its freed centre slot as in Blender (golden `edit/prereq/primitive_merge_circle_ngon_slot`). Auto Merge (`tool_settings.use_mesh_automerge`) is applied after edit-mode vertex transforms (`EDBM_automerge`, golden `edit/transform/automerge`); `use_automerge_and_split=True` (and so `mesh.extrude_manifold`) still raises `OP_NOT_IMPLEMENTED`, now with a hint. The `ed` namespace is dumped (§4.1, group `io`) and OPERATORS.md plans `ed.undo/redo/undo_redo/undo_push` for M8 (WP 8.4), so they answer `OP_NOT_IMPLEMENTED` instead of `OP_UNKNOWN`. Hints: variable-length arrays suggest a list (`indices expects a list, e.g. [5] or [4, 6]`), one-element examples are written `(0,)`, a trailing short-form comma and an unquoted path (`/`, `~`, `.`, `//`) get their own hints. | §4.1, §12, §13.2, §19, §22.4 |
| M2S3-1 | M2 stabilization round 3 (exit-verifier findings) | `BLI_kdtree` balance ported comparison for comparison (`while (co[++i] < pivot)` as `!(a < b)`, `node_test_nearest` as `!(d >= min)`): NaN coordinates no longer run off the node slice, so `mesh.remove_doubles` and Auto Merge answer like Blender (`Removed 0 vertices`; golden `edit/delete/remove_doubles_nan`). `--max-elements` in Edit Mode: the post-statement check counts an edit mesh's BMesh loops; `subdivide`, `subdivide_edgering`, `loopcut`, `spin`, `screw` and `bevel` pre-estimate their corners (`frap-ops-mesh/src/ops/_growth.rs`); BMesh loop allocation calls `budget::check_corners`, which unwinds with an `ElementsExceeded` payload the transaction answers as `LIMIT_EXCEEDED` with rollback, so unestimated growth stops at the limit. Bevel's `interp_vmesh` is a `--stmt-timeout` cancellation point. `frap.render_contact_sheet` warns `frap: no objects to frame (…)` when nothing is framed. `frap run <file>` / `-c` write and flush each response as its statement finishes (`Session::exec_source_each`), and `--fail-fast` stops executing (not just printing) after the first failure. `cargo xtask fmt [--check]` runs rustfmt on every tracked hand-written `.rs` (automod modules included, generated files excluded); CI gates on it. D-77 extended to normals of small/thin faces (tol pinned at 1e-4 by two goldens). | §12, §13, D-77, `frap-geom/src/kdtree.rs`, `frap-math/src/budget.rs`, `xtask/src/fmt.rs` |

## Decision log (M3+M4 W0, WPs 3.0 + 4.0: merged contract delta and stubs)

M3 and M4 run in parallel (§22.11), so one integrator did WP 3.0 and WP 4.0 together on branch `m3-m4-materials-modifiers`, from the M2 stabilization head. It closed the M2 leftovers it could (M34W0-11), assigned the rest, and applied both contract deltas. Every change below is additive unless stated otherwise.

| # | Topic | Decision | Where |
|---|---|---|---|
| M34W0-1 | Codegen | `spec/current_milestone` = 4: typed props for every M3 and M4 operator (new `frap-spec-ops-uv/src/generated/uv.rs`, `frap-spec-ops-object/src/generated/scene.rs`, `frap-spec-ops-data/src/generated/{image,world}.rs`; `object`, `view3d`, `mesh` extended). `xtask/src/codegen/gen_plain.rs`: `Extra` gains `ty` (a typed non-RNA field, `Default::default()`), used for `NodesSettings.inputs`; `TextureFields` (Texture ∪ ImageTexture, like `LightFields`); `ToolSettings` binds `use_mesh_automerge_and_split`; every generated struct gets `id_refs()`/`set_id_ref()` over its ID-pointer fields, dispatched by `ModifierSettings`. `codegen --check` clean. | §4.2 |
| M34W0-2 | Merged wave plan | W1 = 3.1, 3.2, 3.3, 3.4, 3.10, 4.1–4.6; W2 = 3.5, 3.6, 3.7, 3.8, 4.7; W3 = 3.9 (§22.6 "Merged plan"). The M4 golden prerequisite (`modifiers.new`, `object.modifier_add`) is WP 4.1's, in the same wave as 4.2–4.6: they list 4.1 as *(soft)*. W0 made the rest of the path work so that only modifier creation is missing: `frap-eval` evaluates every stack through `frap_modifiers::stack::evaluate`, which dispatches to the registered evaluators (M34W0-5). | §22.5, §22.6 |
| M34W0-3 | Ownership overlaps | Resolved so no two WPs of one wave own a path: `frap-nodes/src/builtin_groups/` split (`smooth_by_angle.rs` 4.1, the rest incl. "glTF Material Output" 3.1); `frap-scene/src/id/*` one owner each (`image` 3.2, `camera`/`light` 3.4, `scene` 3.10, `modifier` 4.1, `texture` 4.2, `pointcloud` 3.7) and `hooks/` by prefix (`image_*`, `camera_*`/`light_*`, `window_*`, `modifier_*`/`object_modifiers_*`, `texture_*`); `frap-ops-data/src/funcs/` by the real stems (`nodes_*`/`node_links_*` 3.1 — the plan's `links_*` —, `blend_data_images_*`/`image_*` 3.2 — the plan's `images_*` —, `blend_data_scenes_*` 3.10, `object_modifiers_*` 4.1 — the plan's `modifiers_*` —, `blend_data_textures_*` 4.2, `mesh_calc_tangents` 3.7); `frap-ops-object/src/ops/` by file (3.1, 3.4, 3.10, 4.1 disjoint); `frap-project/src/scene_map/**` 3.2 in W1, 3.7 in W2 (W0 already persists textures and modifier ID pointers, so 3.10/4.1/4.2 need no scene-map edit); `frap-io-gltf/src/export/{vtree,write}.rs` 3.7 (W2; W0 moved scene gathering to `scenes.rs`, 3.10), `export/{materials/mod.rs,images.rs}`, `import/{materials,images}.rs` 3.5, `export/mesh/gather.rs` 4.7; the plan's `frap-material/src/raise/**` is `lowered/{build,graph}.rs` (3.5; 3.6 keeps helpers inside its pattern files); the plan's `materials/ext_*.rs` and `import/materials_ext_*.rs` are the automod directories `export/materials/ext/` and `import/materials_ext/` (3.6). The §22.5/§22.6 rows say this. | §22.5, §22.6 |
| M34W0-4 | Operators without a WP (rule 1) | `object.material_slot_move` → 3.1; `object.hide_view_set/clear`, `object.make_single_user`, `object.make_links_data` → 3.10 (view-layer and data-sharing area); `view3d.object_as_camera` → 3.4; `Mesh.calc_tangents` → 3.7; `bpy.data.textures.new` → 4.2. OPERATORS.md notes say so. | OPERATORS.md |
| M34W0-5 | `frap-modifiers` contract (§7.2, frozen) | `ModKind` has Blender's five `ModifierTypeType`s; `ModTypeFlags` the `eModifierTypeFlag_*` bits; `ModError { Error, Dependency }`; `ModifierTypeInfo` and `MODIFIER_TYPES` as §7.2; `ModEvalCx` gains `main()`, `object_ref()`, `frame()` (DISPLACE textures, BOOLEAN collections, NODES groups, BUILD). `#[modifier(TYPE, kind = …, flags = A \| B, depends_on = f, is_disabled = g)]` implemented in `frap-macros/src/modifier.rs` (registration + type test). `stack.rs` (walk: pin-to-last, `show_*`, `is_disabled`, evaluator errors as WARNING + `ModifierStatus::Error`, pass-through WARNING) and `registry.rs` (`find`, `duplicates`, `evaluated_types`) are functional; `types/` is an automod directory. `frap-eval/src/geometry.rs` implements `ModEvalCx` (operand meshes and matrices through the cache, a cycle guard, image decoding as the render builder does, the default subdivider/solver) and `cache/mesh.rs` calls it; the fingerprint does not yet include `depends_on` targets (WP 4.1, which owns `cache/{mesh,fingerprint}.rs` and `geometry.rs`). | §7.2, §7.3 |
| M34W0-6 | Backends | `frap-subdiv`: `Subdivider` over plain arrays (`SubdivTopology` in `Mesh`'s layout, `SubdivSettings` = `bke::subdiv::Settings`, `SubdivResult` with vertex/corner interpolation weights and edge/face origins), `CatmullClark` (stub), `osd::OpenSubdiv` behind feature `osd` (`subdiv-osd`), `layout.rs`. `frap-boolean`: `BooleanSolver` (`BoolMesh`, `BoolOptions`, `BoolResult` with face/vertex/corner sources), `manifold::ManifoldSolver` behind feature `manifold` (default on; `boolean-manifold` forwards it). `frap-uv`: `ParamMesh` = `ParamHandle` with Blender's life cycle (`face_add` takes `ParamFace`; `flush` writes into the caller's corner UV array at the indices given), `pack` (`PackParams` = `UVPackIsland_Params` with its defaults, `PackIsland`, `pack_islands`), `project` (`map_to_tube`, `map_to_sphere`, `uv_map_mirror`, cube projection), `smart::project_groups`, doc stubs `lscm`/`abf`/`slim`/`stretch`/`average`. `frap-geom/src/mikktspace.rs` (3.7). `deny.toml`: `opensubdiv-rs` is `MIT AND LicenseRef-TOST-1.0` (Pixar's modified Apache-2.0, the licence Blender ships OpenSubdiv under), allowed for that crate only; the backend stays off by default (WP 4.4's gate). | §3.2, §3.5, §3.6 |
| M34W0-7 | `LoweredMaterial` (§16.2, additive) | `MaterialExts { clearcoat, sheen, transmission, volume, ior, specular, iridescence, anisotropy, dispersion, diffuse_transmission }` with one exhaustive struct per extension (`Channel`s for factor+texture, glTF member names in the docs); `LoweredMaterial::occlusion_strength` (1 by default; the importer reads `occlusionTexture.strength`). The five struct literals outside `frap-material` gained the field. | §16.2 |
| M34W0-8 | glTF extension points | `frap-io-gltf/src/material_ext.rs` (CONTRACT, then 3.6): `MATERIAL_EXT_EXPORTERS` run after the core members in `materials/mod.rs::build` with `ExtExportCx { g, material, mesh }` (`texture_info` through `TextureSlot::Ext(member)`), `MATERIAL_EXT_IMPORTERS` run in `import/materials.rs::lowered` before `raise` with `ExtImportHost` (`texture`, `report`). `TextureSlot` gains `Occlusion` and `Ext`. `ExportSettings::props` (every typed prop) and `SUPPORTED_EXPORT_PROPS` + `is_supported` replace edits of `SUPPORTED_PROPS` (`frap-ops-io` and its test use `is_supported`). `ExportSettings::evaluated_meshes`: with `export_apply=True` the operator evaluates every MESH object (viewport) and the exporter's `apply::evaluated` (4.7) reads them. `Gather::state::<T>()`. New files wired with M1 behaviour: `export/scenes.rs` (the scene push moved out of `write.rs`; `write::finish(g)`), `cameras.rs`/`lights.rs` (`exported` = false, `gather` = none; `nodes.rs` asks for the camera before the children, as the add-on's alphabetical members do), `extras.rs` (none; asked for objects, meshes, materials, scenes), `apply.rs`, `filters.rs`/`formats.rs` (doc stubs). M1 output is unchanged (export tests and goldens green). | §17.2 |
| M34W0-9 | `frap-scene` data | `Texture { header, type, fields: TextureFields, animation_data }` (image pointer counted and remapped), saved/loaded by `.frap`. `NodesSettings.inputs: GnInputs` (`IndexMap<identifier, GnInput { name, value: GnValue, type, attribute_name }>`, serde) for the 5.2 path `mod.properties.inputs.Input_1.value`; its runtime-defined RNA binding is 4.1's. Modifier ID pointers: `Object::for_each_ref`/`remap_ref` include them, `Main::for_each_user_ref` counts the non-object ones (`IDWALK_CB_USER` for node groups, textures, collections; `IDWALK_CB_NOP` for objects, verified in every `MOD_*.cc`), `.frap` writes and reads them (`modifier_j`/`modifier` through `idref`). `scenes.rs` (`new_scene`, `copy_scene` with `SceneCopy`, `remove_scene`) and `window.rs` (`set_scene`) are stubs for 3.10. | §5.6, §7.1, §15 |
| M34W0-10 | `frap-bmesh` and others | `ops/decimate_{dissolve,unsubdivide}.rs` are exact ports of the two Blender wrappers (`BM_mesh_decimate_dissolve` over WP 2.7's `decimate_dissolve_ex`, now `pub` with `Delimit`; `BM_mesh_decimate_unsubdivide` over WP 2.6's `decimate_unsubdivide_ex`); `ops/decimate_collapse.rs` (`DecimateCollapseIn`) and `ops/intersect_edges.rs` are stubs. `frap-render-scene/src/build/evaluated.rs` (4.7) is a doc stub: renders already use `EvalMode::Render` evaluated meshes. `frap-ops-uv` has an automod `ops/` directory. | §6.2, §19.2 |
| M34W0-11 | M2 leftovers | (a) Auto Merge with split: `tool_settings.use_mesh_automerge_and_split` is bound; edit-mode transforms compute `T_AUTOMERGE`/`T_AUTOSPLIT` as `initTransInfo` does (the operator's `use_automerge_and_split` when set — `False` now disables Auto Merge, as in Blender — else the tool settings) and call `edit/automerge.rs::automerge_and_split`, which still answers `OP_NOT_IMPLEMENTED` with the M2 hint (`IMPLEMENTED = false`); the port (`EDBM_automerge_and_split`, `BM_mesh_intersect_edges`) and `mesh.extrude_manifold` go to WP 4.6. (b) `mesh.bevel` and `mesh.symmetrize` apply Auto Merge (`EDBM_automerge_connected`: `find_doubles(use_connected=True)` + `weld_verts`, `frap-ops-mesh/src/ops/_automerge.rs`); D-79 narrowed to `harden_normals`, the §2 gaps updated; goldens `edit/bevel/automerge`, `edit/misc/symmetrize_automerge`. (c) `render.render` reports unrendered Workbench effects (cavity, depth of field) whatever the engine, since EEVEE and CYCLES fall back to Workbench with the scene's display shading (`frap-ops-render-tests` asserts it for both). (d) The M2 W0 CI fixes (lavapipe/WARP in `ci.yml`'s test job, Windows `cfg(all(test, unix))`, `deny.toml`) are still not run on GitHub (nothing is pushed); the new dependencies are pure Rust (no cmake/libclang: the cc-only job holds), and new GPU tests must use the adapters of §20.5 (Metal, lavapipe, WARP) and `FRAP_GPU_OPTIONAL`. | §10.2, §19.4, deviations D-79, §2 |
| M34W0-12 | Dependencies | `frap-uv`: frap-math, frap-geom, glam, libm, faer, smallvec, rustc-hash, thiserror. `frap-subdiv`: frap-math, glam, libm, smallvec, rustc-hash, thiserror, opensubdiv-rs (optional). `frap-boolean`: frap-math, glam, libm, smallvec, rustc-hash, thiserror, manifold-rust (optional, default). `frap-modifiers`: frap-rna, frap-math, frap-macros, frap-geom, frap-mesh, frap-nodes, frap-bmesh, frap-scene, frap-imbuf, frap-subdiv, frap-boolean, glam, libm, linkme, automod, bitflags, smallvec, smol_str, indexmap, rustc-hash, thiserror. `frap-eval`: + frap-modifiers, frap-subdiv, frap-boolean, frap-imbuf. `frap-ops-uv`: frap-op, frap-rna, frap-scene, frap-spec-rna, frap-spec-ops-uv, frap-mesh, frap-bmesh, frap-uv, frap-geom, frap-math, glam, libm, linkme, automod, smallvec, smol_str, rustc-hash. `frap-ops-object`: + frap-modifiers, frap-eval, frap-nodes, smallvec. `frap-ops-data`: + frap-imbuf, frap-color, frap-modifiers, frap-bmesh, frap-geom, frap-math, smallvec. `frap-ops-mesh`: + frap-boolean, frap-modifiers. `frap-geom`: + bevy_mikktspace. `frap-io-gltf`: + smallvec. `frap-ops-io`: + frap-mesh. `frap-ops-all`: + frap-subdiv, frap-boolean (features), frap-modifiers (`frap_modifiers::link()` is referenced by `frap_ops_all::link()` like the ops crates, so `types/<type>.rs` registrations survive LTO; `tests/registry.rs` checks in release that every `types/<type>.rs` registers its upper-cased stem exactly once), dev frap-uv, frap-geom. `check-deps` passes. `deny.toml` (CI runs `cargo deny check --all-features`): the `opensubdiv-rs` licence exception and an advisory ignore for `paste` (RUSTSEC-2024-0436, unmaintained; a build-time proc-macro reached through faer → gemm, no runtime code). | §3.2, §3.5 |
| M34W0-13 | Contract pins | `crates/frap-ops-all/tests/contracts.rs` pins `ModifierTypeInfo`/`ModEvalCx`/`stack`/`registry`, `Subdivider`, `BooleanSolver`, `ParamMesh`/`pack`/`project`/`smart`, `mikktspace`, the decimate and intersect-edges ops, `MaterialExts`/`occlusion_strength`, the scene/window functions, `Texture`, `GnInputs`, `id_refs`, the material-extension slices, `SUPPORTED_EXPORT_PROPS`/`is_supported` and `ExportSettings::{props, evaluated_meshes}`. | §3.4 |

## Decision log (M3+M4 W1 integration)

The eleven W1 packages (3.1 node trees and world, 3.2 images, 3.3 `frap-uv`, 3.4 cameras and lights, 3.10 scenes, 4.1 modifier stack core, 4.2 to 4.6 modifier types and backends) were merged into `m3-m4-materials-modifiers` with `git merge --no-ff` in the order 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 3.1, 3.2, 3.3, 3.4, 3.10 (4.1 first: the M4 goldens need its `modifiers.new`). Every change a WP made outside its §22.5/§22.6 paths, and every addition to a frozen contract, is recorded here; Blender deviations went to `docs/deviations.md` (D-86 to D-101, D-79 extended, §2 gaps).

| # | Topic | Decision | Where |
|---|---|---|---|
| M34W1-1 | Merge conflicts | `frap-eval/tests/evaluator.rs` (4.2, 4.3, 4.4 each swapped the pass-through test's now-evaluated types): kept WP 4.2's version, which uses only types no M4 WP evaluates (`REMESH`, `SKIN`, `UV_PROJECT`). `frap-schema/tests/snapshots.rs` `manifest_selected_operators` (4.1 swapped `object.modifier_add` for `object.modifier_apply_as_shapekey`, 4.5 swapped `mesh.decimate` for `mesh.select_similar`): both new entries kept. `docs/OPERATORS.md` (4.1 and 3.2 ticked adjacent rows): both ticks kept. `crates/frap-modifiers/src/types/_dir.rs` and `frap-nodes/src/builtin_groups/_dir.rs` deleted by several WPs (clean delete/delete). `frap-project/src/scene_map/{read,write,tests}.rs` (3.1, 3.2, 4.1) and `frap-session/src/resolve.rs` (3.2, 4.1) merged without textual conflict. | §22.5, §22.6 |
| M34W1-2 | WP 4.1 non-owned edits (accepted) | `frap-session/src/resolve/access.rs`: `with_struct` reads a pointer rooted at an object with path `[data, …]` as that object's evaluated mesh, through a thread-local `EvalCache` (capped at 64 entries). `frap-session/src/resolve.rs`: `ob_eval.data` of a mesh object with modifiers resolves to `objects['X'].data` (`Detached::Temporary`; `.original` is itself, `is_evaluated` False). `frap-ops-data/src/funcs/object_to_mesh.rs` (evaluated object → evaluated mesh; the M1 `NotImplemented` path is gone) and `i_d_evaluated_get.rs` (stack warnings as WARNING reports). `frap-ops-object/src/support/shade.rs`: `shade_smooth`/`shade_flat` remove the "Smooth by Angle" modifier, as Blender. `frap-project/src/scene_map/read.rs`: `builtin_group_node`, so `.frap` files holding the linked Smooth by Angle group load. `tools/blender/golden_runner.py` `NORMALISE["D-85"]`. New test files `frap-eval/tests/modifier_stack.rs`, `frap-session/tests/m4_modifier_stack.rs`. Additive API: `frap_modifiers::registry::{meta, TypeMeta, object_supports, object_supports_modifiers, addable_types}`, `stack::{edit, type_of, evaluate_one, pass_through_message, BAD_STACK_POSITION, is_frap_limitation, status_is_frap_limitation}`, `types::nodes::{is_smooth_by_angle_modifier, smooth_by_angle}`, `GnValue::None`, `GnInput::new`, `Modifier::{DEFORM_ONLY_TYPES, REQUIRES_ORIGINAL_DATA_TYPES, index_in, ptr, set_pin_to_last}`, `EvalCache::apply_modifier`; the prelude re-exports `smallvec::SmallVec`. | §7.2, §7.3, §9 |
| M34W1-3 | WP 3.1 contract additions and non-owned files (accepted) | Frozen `NodeTree` gains `pub next_interface_uid: u32` (Blender's `bNodeTreeInterface.next_uid`; interface identifiers `Socket_<n>` are never reused; §16.1 listing updated); `.frap` scene map writes it as an optional node-tree key when non-zero. Ownership additions: `frap-ops-data/src/funcs/blend_data_node_trees_{new,remove}.rs`, `funcs/node_tree_interface_{new_socket,remove,clear}.rs`, `hooks/{_nodes,shader_node_group_node_tree,node_tree_interface_socket_name,node_tree_interface_socket_socket_type}.rs` are WP 3.1's (no WP owned them; "node trees complete" needs them); `funcs/blend_data_worlds_new.rs` edited (colour 0.05 as `BKE_world_add`, was the factory 0.0509); `funcs/_common.rs` `socket_of` resolves keys with `frap_nodes::rna::find_socket_by_key`. Additive `frap-nodes` API: group items at the crate root (`GROUP_INPUT`, `GROUP_OUTPUT`, `EXTEND_SOCKET`, `is_group_node`, `interface_socket_type/idname/default`, `interface_socket_types`, `SHADER_/GEOMETRY_INTERFACE_SOCKET_TYPES`, `Node::{sync_group_sockets, group_tree, prop_bool}`, `NodeTree::{interface_new_socket, interface_remove, interface_clear, interface_rename, interface_set_socket_type, sync_group_io_nodes, sync_group_node, sync_group_nodes_of, add_group_node}`), `builtin_groups::gltf_material_output`, `rna::{NodeInfo, node_info, socket_class_labels, find_socket_by_key}`, `rna::interface` (per-tree-type interface StructDefs: `NODE_TREE_INTERFACE` is the shader variant, `GEOMETRY_NODE_TREE_INTERFACE`, `item_srna(tree_ty, socket_ty)`, `interface_srna`, `socket_type_of`, `SET_PREFIX`). §16.1's "Iridescence Thickness Maximum" glTF Material Output input does not exist in the 5.2.4 add-on: frap follows the add-on (5 inputs). | §16.1, §15 |
| M34W1-4 | WP 3.2 contract additions and non-owned files (accepted) | `frap-scene/src/host.rs`: `HostRead::cwd(&self) -> Option<&Path>` (default `None`); `frap-session` `HostImpl` stores `SessionOptions::cwd` and answers it (relative paths of `images.load`/`Image.save`; golden sessions run in-process with their own cwd). `frap-scene/src/id/mod.rs` re-exports `ImBuf`, `ImBufData`, `ImageDecoder`, `IMAGE_DECODERS`, `LoadError`, `PathBase`, `decode_image`, `io_message`, `ImInfo`, `ImageProbe`, `IMAGE_PROBES`, `probe_image` and the `imbuf` module. `Image` gains `packed_filepath`, `generated_depth`, `buffer: Option<Arc<ImBuf>>` and `acquire(&PathBase)` (rows bottom-first, colour handling applied there), `load`, `generated_buffer`, `known_size`, `is_dirty`, `free_buffers`; `ImBuf::{scale, flip_rows}`, `ImBuf.colorspace`. `frap-math` budget: `ElementsExceeded.buffer`, `BUFFER_BYTES_PER_ELEMENT` (32), `reserve_buffer`, `check_buffer` (image buffers count against `--max-elements` × 32 bytes; the session reports detail `bytes`). `frap-session/src/resolve.rs`: `long_array_json`/`long_value_json`; dynamic arrays longer than `--max-array` are answered as a capped `$array`, assignments echo them the same way. `frap-imbuf`: `probe(&[u8]) -> ImageInfo` (header only). `frap-op`: `PollReq::ContextImage` (`CONTEXT_IMAGE`), named in `frap-schema`. Readers outside WP 3.2 (render build, `frap-eval` DISPLACE images, glTF export images) still decode files themselves: assigned to WP 3.5 (export, W2) and the render/eval owners (deviations §2). | §5, §12, §13 |
| M34W1-5 | WP 3.3 `frap-uv` (owned) | `PackParams::default()` matches `UVPackIsland_Params` (rotate `None`, pin `None`, shape `Aabb`; margin 0.001); `PinMethod::None`; `uv_map_mirror(face_uvs, regular, fan)` as Blender; `PackIsland::build_transformation` is column-major (`m[col][row]`). New public items (`ParamMesh::{prepare_pin_index, find_pin_index, chart_has_pins, non_finite_charts}`, `pack::{apply_to_uvs, box_pack_2d, convexhull, …}`, `project::{map_to_tube_ex, map_to_sphere_ex, uv_map_transform, curved_project, …}`, `smart::{AREA_IGNORE, project_face}`, `param::bli` with an Apple-compatible `atan2f`, D-94). For WP 3.8: the operator-pipeline glue (`bm_mesh_calc_uv_islands`, `uvedit_pack_islands_multi` with pin skipping, `construct_param_handle` with pin index and seam keys, smart-project zero-area handling) is still test-only in `crates/frap-uv/tests/common/mod.rs` and must be ported into `frap-ops-uv`; after unwrap, call `ParamMesh::non_finite_charts()` and report NaN charts; a bare `uv.unwrap()` takes `ANGLE_BASED`/`fill_holes=True` from the tool settings and `smart_project` writes `uvcalc_margin` (research doc §4.15 corrected). CI: the release step also runs `cargo test --release -p frap-uv --test blender_goldens --test pack_invariants` (packer goldens are ignored in debug builds). | §3.6, §22.5 |
| M34W1-6 | WP 3.4 non-owned edits (accepted) | `frap-io-gltf/src/export/vtree.rs` (3.7's in W2): `VNode.matrix_world` goes through `cameras::correct_matrix_world(settings, ob, m)` (the Y-up camera/light correction). `frap-scene/src/startup.rs`: one call to `Light::set_factory_startup_hidden_fields()` before `load_rna` (the startup dump lists only PointLight props, so the factory light's spot size, angle, area shape/size and cascade distance differed after re-typing). New API: `Light::{temperature_color, area, set_factory_startup_hidden_fields}`, `light::blackbody_temperature_to_rgb`, `export::cameras::{correction, correct_matrix_world, camera_to_gltf, yfov_blender_to_gltf}`, `export::lights::{light_to_gltf, PBR_WATTS_TO_LUMENS}`; `SUPPORTED_EXPORT_PROPS` gains `export_cameras`, `export_lights`, `export_import_convert_lighting_mode`. The negative-scale camera fit differs by Apple's `atan2f` (D-94; fixing it means emulating it in `frap-math/src/euler.rs`). Edit-mode fits with modifiers shown in Edit Mode need a golden now that `modifiers.new` exists (follow-up). | §17, §5 |
| M34W1-7 | WP 3.10 non-owned edits (accepted) | `frap-scene/src/hooks/id_users.rs`: `ID.users` reports at least 1 for every scene (Blender), not only the window's. `frap-scene/src/data.rs`: runtime field `Main::base_selected: layers::BaseSelectedMemory` (Debug prints a constant; not saved in `.frap`), standing in for `ob->base_flag & BASE_SELECTED` (D-95). `frap-scene/src/id/layers.rs`: `BaseSelectedMemory`; `synced_view_layers` takes it (no outside callers); `select_set` and every `sync_scene`/`sync_all` maintain it. `frap-ops-data/src/funcs/collection_objects_link.rs`: doc comment only. `frap_scene::scenes` gains `SceneCopy::identifier`, `NEW_SCENE_OVERLAY`, `default_scene`, `duplicate_id`, `relink_ids`, `can_be_removed`, `find_replacement`, `last_scene_message`; `frap_scene::window::view_layer_for` (W0 signatures unchanged). Follow-ups: `Camera::for_each_ref`/`remap_ref` (3.4's file) do not walk `dof.focus_object` (3.10 relinks it explicitly in `scenes.rs`); `make_single_user` on thousands of objects is slow in debug builds (linear passes in `Scene::remap_ref` and view-layer sync). | §5.6 |
| M34W1-8 | WP 4.2 to 4.6 non-owned edits (accepted) | 4.2: helper submodules under owned files (`types/weld/{merge,util,vgroup_tests}.rs`, `types/displace/tex.rs`, `types/solidify/{extrude,nonmanifold}.rs`; MIRROR and ARRAY use `super::weld`). 4.3: `frap-bmesh/src/ops/wireframe.rs` gains `WireframeVgroup` and `mesh_wireframe_ex` (vertex-group part of `BM_mesh_wireframe`; `mesh_wireframe` unchanged); fixtures `tests/golden/fixtures/modifiers/b/vgroups/` hold Blender dumps read by unit tests; `screw::screw()` returns `Result`. 4.4: `SubdivResult` gains `#[non_exhaustive]` `vert_sources`/`corner_sources` (`DataSource`) and `SubdivResult::identity`; `layout::{Layout, Coarse, compute}`, `SubdivTopology::coarse`, `FVarLinear::from_uv_smooth`, `VtxBoundary::from_boundary_smooth`; `FVarLinear` and `SubdivSettings::default().fvar_linear` default to `Boundaries` (5.2's `PRESERVE_BOUNDARIES`; was `CornersOnly`); `SubdivSettings::is_adaptive` is Cycles' adaptive subdivision (Blender's `is_adaptive` maps to `use_limit_surface`); `edge_creases` are copied to child edges, not decayed. 4.5: `frap-bmesh` bevel gains `MeshBevelWeights` and `mesh_bevel_tagged` (vertex-group weights applied before `use_weights`, as `BM_mesh_bevel`), `DecimateCollapseIn::with_factor`; `frap-mesh` `Mesh::lookup_float` (Blender's `lookup<float>`, new private `Kind::ByteRaw`); `xtask/src/codegen/gen_plain.rs` `MODIFIER_ALIASES` and an `aliases` parameter on `emit_struct` (an alias property has no field and reads/writes its target: `BevelModifier.width_pct` ↔ `width`), `generated/modifier_settings.rs` regenerated (`codegen --check` clean). 4.6: `frap-bmesh` `contextual_create/kdopbvh.rs` gains `BvhTree::{leaf_num, overlap_ex}` (port of `BLI_bvhtree_overlap_ex`, used by `BM_mesh_intersect_edges`); `frap-boolean` additions (`BoolMesh::{corner_tris, with_transform, with_corner_tris, faces_num, is_identity, transformed_positions}`, `BoolError::{NonManifold, ResultTooBig, Unknown}`, modules `assemble`, `knife`); `frap-session/tests/m2_stabilization.rs` `auto_merge_with_split_works`; `tests/golden/cases/edit/extrude/macros/manifold.py.pending` enabled; describe snapshots of `mesh.extrude_manifold`/`intersect`/`intersect_boolean`/`decimate` re-blessed. | §6.2, §7.2, §3.5 |
| M34W1-9 | Integration edits | `frap-eval/src/geometry.rs` (4.1's): the stack's cycle guard (`EvalCache::in_progress`) is cleared when a modifier unwinds (`TIMEOUT`, ARRAY's `LIMIT_EXCEEDED`): `catch_unwind` + `resume_unwind` around `stack::evaluate`/`evaluate_one`, since the thread-local cache outlives the rolled-back statement and would otherwise report a false "Dependency cycle". `docs/OPERATORS.md`: the M3/M4 operators and modifier types implemented in W1 ticked (`mesh.decimate`/`intersect`/`intersect_boolean`/`extrude_manifold`, `object.material_slot_move`/`hide_view_*`/`make_single_user`/`make_links_data`, `scene.new`/`delete`, `world.new`, the 21 M4 modifier types, Texture, scenes data API, multi-scene/camera/light glTF). `docs/deviations.md` D-86 to D-101, D-79 extended to the BEVEL modifier, §2 gap rows (Auto Merge with split removed; BEVEL CUSTOM profile, `DecimateModifier.face_count`, texture ramps, interface socket types, image packing, image readers, `--verbosity full` `modifier_status`); `golden_runner.py::NORMALISE` mirrors them. | §20.3 |
| M34W1-10 | Open follow-ups (not blocking) | `--describe` `modifier_types[].evaluator` should read `frap_modifiers::registry::evaluated_types()` (WP 4.7); RNA pointer hooks rejecting self-assignment and non-mesh caps (D-97) and a `SimpleDeformModifier` factor/angle alias hook (D-98) (WP 4.1 files); `frap_modifiers::stack::enabled` does not check `eModifierTypeFlag_SupportsEditmode`; `Mesh::lookup_bool`'s BYTE_COLOR path averages as linear floats where Blender mixes raw bytes (`ColorGeometry4bMixer`); `crates/frap-session/tests/limits.rs` `stmt_timeout_cancels_quadratic_mesh_kernels` has a load-sensitive 6 s bound. | — |
| M34W1-11 | Gate | `cargo build`/`test` (1929 tests)/`clippy -D warnings`/`fmt --check`, `cargo xtask fmt --check` (45 hand-written files the WPs left unformatted were reformatted), `check-deps`, `codegen --check`, the release registry/transcripts and `frap-uv` packer tests, and the golden suite against Blender 5.2.4 (`cargo xtask golden live`: 804 cases; `frap-cli` `blender_live`) are green in the main tree. | §20.5 |

## Decision log (M3+M4 W2 integration)

The five W2 packages (3.5 material lowering core, 3.6 `KHR_materials_*`, 3.7 mesh attributes and primitive modes, 3.8 UV operators, 4.7 modifier integration) were merged into `m3-m4-materials-modifiers` with `git merge --no-ff` in the order 3.5, 3.6, 3.7, 3.8, 4.7 (3.5 first: 3.6 and 3.7 build on its lowering helpers and `VertexColorUse`). Every change a WP made outside its §22.5/§22.6 paths, and every addition to a frozen contract, is recorded here; Blender deviations went to `docs/deviations.md` (D-102 to D-117, D-19 extended, §2 gaps).

| # | Topic | Decision | Where |
|---|---|---|---|
| M34W2-1 | Merge conflicts | `frap-io-gltf/src/import/materials.rs` (3.5's `link_settings_group` and 3.6's `material_ext::bind_settings_nodes` both bound the "glTF Material Output" group node after `raise`): kept 3.6's call and removed 3.5's duplicate. `export/materials/mod.rs` module doc: kept 3.5's (it already names the `ext/` exporters). `docs/OPERATORS.md`: both WPs' glTF ticks kept. 3.7, 3.8 and 4.7 merged without textual conflict. The two settings-node creators (`RaiseCx::settings_node`, 3.5; `ext_common::settings_node`, 3.6) both find an existing node first, so a material needing occlusion and volume/iridescence gets one node. | §16.2, §17 |
| M34W2-2 | WP 3.5 contract changes (accepted) | `ExportSettings.cwd: Option<PathBuf>` (`None` by default), set by `export_scene__gltf.rs` from the session's cwd, so image paths resolve as `Image::acquire` resolves them. `VertexColorUse` gains `color`, `alpha`, `alpha_attribute` and `color_and_alpha()`/`color_only()`/`adds_alpha(AlphaMode)` (struct literals must set them); the importer builds it with the active attribute (`None`), alpha only for non-OPAQUE materials. `export/images.rs`: `Role`, `TexInput`, `slot_inputs`, `gather_texture_from`, `gather_texture_info_from`; `Ext` slots fill fixed channels (clearcoat R; clearcoatRoughness and thickness G; specular and sheenRoughness A). `frap-material` graph helpers (`In`, `Konst`, `ColorInfo`, `TexHit`, `PathLink`, `SrcChannel`, `math_op`, `FoldVal`, the `LowerCx` `NodeNav` ports, `folded_value`/`inline_value`/`inline_fails`/`has_available_link_cycle`; `material_output()`/`gltf_output_input()` are `None` on a link cycle), submodule `lowered/graph/fold.rs` (the inliner's constant folding), `RaiseCx::{scalar_factor_and_texture, normal_map, settings_node}`; `texture_transform::from_mapping(&LowerCx, …)`. Pattern orders: unlit 1, sampler 2, uv_source 3, texture_transform 4, emission 10, emission_strength 11, alpha 15, vertex_color 17, base_color 20, metallic_roughness 40, normal_map 42, occlusion 44, double_sided 50; layout blocks normal 40, occlusion 150. `MATERIAL_EXT_EXPORTERS` run between the emission and normal members (the add-on's alphabetical traversal), narrowing M34W0-8's "after the core members". Test-only edits to `frap-material/src/lowered/tests.rs`, `frap-io-gltf/tests/{export_static,import_vs_blender}.rs` (the `UNLIT_UNTIL_M3` skip list is gone). | §16.2, §17.2 |
| M34W2-3 | WP 3.6 contract changes (accepted) | `material_ext.rs` (additive): `ext_of`, `import_texture`, `import_normal`, `bind_settings_nodes` (binds the group nodes `raise` creates to the `bpy.data.node_groups` datablock, creating it when missing). `export/materials/mod.rs` (3.5's file): `textures()` includes every `KHR_materials_*` texture and is the `MaterialCache.written` key. Public constants `clearcoat::BLENDER_COAT_ROUGHNESS`, `ior::GLTF_IOR`, `iridescence::{GLTF_IRIDESCENCE_IOR, THICKNESS_MINIMUM, THICKNESS_MAXIMUM}`, `volume::absorption_node`. Pattern orders volume 2, clearcoat 300, transmission 310, dispersion 320, specular 340, anisotropy 350, sheen 360, iridescence 370, ior 380. Shared helpers in `patterns/transmission/ext_common.rs` (per M34W0-3), Blender fixture dir `patterns/transmission/import_ext/`. Placeholder `_dir.rs` files deleted. | §16.2, §17.2 |
| M34W2-4 | WP 3.7 non-owned edits (accepted) | `frap-mesh`: private `Mesh` tangent layer `Option<(session_uid, Arc<Vec<[f32; 4]>>)>` (temporary `CD_MLOOPTANGENT`, not persisted, read only by the mesh it was computed on; `clear_geometry()` drops it, so BMesh → Mesh conversions do) with `corner_tangents()`/`set_corner_tangents()`; `rna/mesh_loop.rs` `tangent`/`bitangent_sign`/`bitangent` read it; `Mesh::lookup_float2`. `frap-io-gltf`: `Gltf.mesh_cache` values are `IdAny` (mesh or point cloud); `import/nodes.rs` calls `mesh::create_data`/`data_name`; `export/nodes.rs` sends POINTCLOUD objects to `extract::pointcloud::gather` (one match arm; `gather.rs` untouched); `cameras.rs`/`lights.rs` (3.4) put `extras::id(…)` on cameras and lights; `extras::{generate, id, to_json, BLACK_LIST}`, `formats::extension()`, `filters::SPECIAL_COLLECTION`; `SUPPORTED_EXPORT_PROPS` gains the extras, tangents, edges/vertices and vertex-colour props. `frap_scene::id::PointCloud` is a full type (`points`, `attributes`, `materials`, `animation_data`; RNA binds `points`); `.frap` saves point clouds in full. `frap_geom::mikktspace::{corner_tangents, calc_uv_tangent_tris_quads, TangentError}`. `tests/import_behaviour.rs`: points-only meshes become point clouds. | §5.6, §15, §17 |
| M34W2-5 | WP 3.8 non-owned edits (accepted) | `xtask/src/codegen/gen_plain.rs`: a typed extra's `default` is a Rust expression; `ToolSettings` gains the DNA-only `unwrapper` (`'ANGLE_BASED'`), `uvcalc_flag` (1 = fill holes), `uvcalc_margin` (0.001), `uvcalc_iterations` (10), `uvcalc_weight_factor` (1.0), `uvcalc_weight_group` (`'uv_importance'`); `generated/plain/tool_settings.rs` regenerated. `frap-scene/src/scenes.rs` `template()`: new scenes get `unwrapper='CONFORMAL'`, `uvcalc_flag=0` (DNA defaults; verified on 5.2.4). `frap-schema` snapshots: the "not implemented" example is `uv.minimize_stretch` (M8). `frap-ops-uv/src/ops/_dir.rs` deleted. `tests/cli/m3/uv/{extensions_and_errors,unwrap_workflow}.*` (under WP 3.9's `tests/cli/m3/**`) accepted as they are; WP 3.9 owns them from W3. | §4.2, §22.5 |
| M34W2-6 | WP 4.7 non-owned edits (accepted) | `frap-render-scene/src/build/mod.rs`: `Builder::object` takes slots and materials from `evaluated::slot_materials(ob, data, evaluated)` (`BKE_object_material_get_eval`: an empty OBJECT-linked slot falls back to the data's material); the unused `slot_material` is removed and `build/tests.rs` updated. `frap-ops-render-tests/tests/render_goldens.rs`: `AREAS` includes `modifiers/integration`. `frap-schema` (M34W1-10): depends on `frap-modifiers` (L11 → L7, `check-deps` passes); `modifier_types[].evaluator` reads `registry::evaluated_types()` (22 types true). `export/apply.rs` registers `export_apply` and adds `has_modifiers`, `evaluated_materials`, `viewport_evaluated_objects(&Main, &Context)`; with `export_apply` the operator evaluates only the objects the viewport depsgraph evaluates (narrowing M34W0-8's "every MESH object", D-117). | §7.3, §17.2, §19 |
| M34W2-7 | Integration edits | (a) `export/mesh/writers/color.rs` (3.7) reads 3.5's `VertexColorUse` in full: colour from `color`/`attribute`, alpha from `alpha`/`alpha_attribute`, so `export_vertex_color='MATERIAL'` writes VEC4 when Alpha reads the attribute on a non-OPAQUE material and nothing for an Alpha-only material (the add-on does not manage it); `materials_core/fake_color0` enabled and blessed, new golden `gltf/attributes/vertex_colors_material` (3.7's probe, five cases). (b) The `KHR_materials_*` patterns read the inliner's constant folding as 3.5's core patterns do: `LowerCx::folded_output` (graph.rs) and `ext_common::{konst_f32, konst_rgb}`, used by `get_factor`, `scalar_member`, ior, sheen, specular, volume thickness, iridescence thickness and anisotropy; goldens `gltf/materials_ext/{folding,folding_misc}` (Math nodes into Transmission Weight, IOR, Roughness and Coat Roughness). (c) `cargo xtask fmt` over the WP files left unformatted. (d) `docs/OPERATORS.md`: glTF rows for non-triangle modes and UV sets/vertex colours/tangents/extras ticked (3.7). | §16.2, §17 |
| M34W2-8 | Open follow-ups (not blocking) | Material lowering does not look inside node groups or skip muted nodes on a texture's path; texture nodes, Color Ramp, curves, Vector Rotate, Blackbody and Radial Tiling are not folded (M8, deviations §2). A constant-folded chain into an Image Texture's `Vector` input is not treated as unlinked. `materials_ext` goldens use uniform black images: goldens with distinct per-channel pixels should exercise channel routing and packing (clearcoat R/G sharing, thickness G, Separate Color, Alpha). Attribute writers need a report channel and `gather.rs` should skip identity POINTS indices (deviations §2). `frap_uv::pack`'s pre-rotation float rounding (D-115) is not emulated. `export_apply` geometry dependencies through geometry nodes are not read. The `frap-golden` glb comparator does not check accessor sharing in general (covered by a unit test on the `export_apply_hidden` fixture). Shader node rows of OPERATORS.md §C stay unticked until the renderer reads them too. | — |
| M34W2-9 | Gate | `cargo build`/`test` (2042 tests)/`clippy -D warnings`/`fmt --check`, `cargo xtask fmt --check`, `check-deps`, `codegen --check`, the release registry/transcripts and `frap-uv` packer tests, and the golden suite against Blender 5.2.4 (`cargo xtask golden live`: 875 cases; `frap-cli` `blender_live`) are green in the main tree. | §20.5 |

## Decision log (M3+M4 W3 integration)

WP 3.9 (M3 suite) was merged into `m3-m4-materials-modifiers` with `git merge --no-ff`, without textual conflict. It adds no Rust code: 40 golden cases under `tests/golden/cases/gltf/m3_workflows/` (60 first exports, each followed by a frap-vs-Blender round trip of its own output) with their blessed Blender fixtures, and the M3 CLI transcripts under `tests/cli/m3/{gltf,images,materials,scenes,short_form}/`. Blender deviations went to `docs/deviations.md` (D-118, D-119; §2 rows for PNG encoder bytes and the `smart_project` qsort tie).

| # | Topic | Decision | Where |
|---|---|---|---|
| M34W3-1 | WP 3.9 non-owned edits (accepted) | `docs/deviations.md`: D-118 (`texCoord: -1` on a mesh without UV maps: frap omits it, schema-valid; normalised in the `#@blender` line of `export_options.py`) and D-119 (custom-normal fan spaces of a re-exported import within 1e-4; `modifier_stack_reimport.py` compares at `tol=2e-4`), §2 rows. `tools/blender/golden_runner.py` (WP 1.8): `NORMALISE` entries for D-118 (`cases`) and D-119 (`comparator`), as `normalisation_list_matches_deviations` requires. `tests/data/gltf-roundtrip/allow.toml` (WP 1.23): eight Khronos samples with M3 content (MeshPrimitiveModes, TextureTransformTest, MultipleScenes, Cameras, PointLightIntensityTest, BoxVertexColors, UnlitTest, EmissiveStrengthTest) and their blessed Blender re-exports under `tests/golden/fixtures/gltf/m1/roundtrip/`; LightVisibility is left out (Blender 5.2.4's add-on refuses its required `KHR_node_visibility`). | §17.2, §20.5 |
| M34W3-2 | Open follow-ups (not blocking) | `frap-uv/src/pack/qsort.rs` (BLI_qsort_r port) switches to insertion sort below 7 elements where macOS libc's `qsort` (which Blender's `smart_project` calls) does so up to 7; even with threshold 8, 2 of 29500 random arrays sort differently, so macOS `qsort` is not exactly FreeBSD's. `frap-bmesh`'s `c_qsort` (D-67) and `frap-ops-uv`'s lightmap `qsort.rs` share the algorithm. Fixing it would let `hex_cone_atlas.py` move to T2. The 1e-4 `smart_project` difference on the `tangent_space.py` ico sphere is not root-caused. D-118 is a documented deviation: byte identity would need a signed `texCoord` in `frap-gltf` and `-1` from `frap-io-gltf`. JPEG passthrough has no golden (canonical comparison hashes JPEG bytes and scratch-dir cases cannot reach a committed JPEG input). The ≥ 60 static-glTF golden count of the M3 exit criterion is counted by hand (60 first exports), not by a test. | §22.5 |
| M34W3-3 | Gate | `cargo build`/`test` (2042 tests)/`clippy -D warnings`/`fmt --check`, `cargo xtask fmt --check`, `check-deps`, `codegen --check`, the CLI transcripts (`tests/cli/**`, including `tests/cli/m3/**`) and the golden suite against Blender 5.2.4 (`cargo xtask golden live`: 915 cases; `frap-cli` `blender_live`) are green in the main tree. | §20.5 |

## Decision log (M3+M4 stabilization round 1)

Fixes for the M3+M4 exit verifier's findings, on `m3-m4-materials-modifiers`.

| # | Topic | Decision | Where |
|---|---|---|---|
| M34S-1 | BOOLEAN modifier goldens | The five `tests/golden/cases/modifiers/boolean/{exact,float,manifold_difference,manifold_union_transfer,manifold_collection}.py.pending` (waiting for WP 4.1, which landed in W1) are enabled and blessed: EXACT (the default solver, T3 invariants), FLOAT (T3), MANIFOLD DIFFERENCE, UNION with `material_mode='TRANSFER'`, and the COLLECTION operand (T2). | §22.6 |
| M34S-2 | `KHR_materials_pbrSpecularGlossiness` import | Ported (`io_scene_gltf2` 5.2.41 `imp/KHR_materials_pbrSpecularGlossiness.py`): `frap-material` gains `MaterialExts::pbr_specular_glossiness: Option<SpecularGlossiness>` (additive; `MaterialExts` is `non_exhaustive`) and the pattern `patterns/pbr_specular_glossiness.rs` (order 0, raise only), which builds the whole importer graph (diffuse through `raise_base_color` with the label `DIFFUSE`, which gains a `label` parameter; emission, normal, occlusion with the settings node at (10, 425); `IOR` 1000; `Specular Tint`; `Roughness` = 1 − glossiness or the baked image's `Alpha`) and then clears `RaiseCx::bsdf`, so the later patterns add nothing; `emission`/`normal_map`/`occlusion` raisers are `pub(crate)`. The importer reads the extension from the raw JSON (no typed `frap-gltf` struct, so §17.1's "pbrSpecularGlossiness (read)" is untyped), skips `pbrMetallicRoughness` and the other material extensions as the add-on does, and bakes `make_roughness_image` (a packed copy `<image>.001`, alpha 1 − factor × A in float32, cached per glTF image whatever the factor: `Gltf::roughness_images`). The stale "imported from M3; it was ignored" warning is gone. Golden `gltf/materials_core/spec_gloss.py` (T1 materials + the baked image's pixels); its input is the first use of case input directories (§20.2, `frap_golden::blender::stage_inputs`, called by both runners). The add-on fails (`KeyError: 'KHR_materials_specular'`) on a `specularGlossinessTexture` with `KHR_texture_transform`; the golden avoids it. | §16.2, §17.2, §20.2 |
| M34S-3 | Workbench `VERTEX`/`TEXTURE` colour types | No M3 WP owned the renderer, so the deviations row that named M3 is moved to M5 (WP 5.5, with the texture and colour-attribute upload the EEVEE-like renderer needs; HDRI world WP 5.2). Until then `render.render` and `frap.render_contact_sheet` report a WARNING when `shading.color_type` is `VERTEX` or `TEXTURE` (outside wireframe mode), so agents know the render shows the material colour (`frap-ops-render` `color_type_left_out`). Blender 5.2.4's `color_type` items are MATERIAL, OBJECT, RANDOM, VERTEX, TEXTURE, SINGLE (no `ATTRIBUTE`). | §19.3, deviations §2 |
| M34S-4 | deviations §2 rows past their milestone | Retargeted with owners: glTF reader enum values and natural-sort ties → M7; the eight non-shader `nodes.new` types, `is_active_output` writes, preferences writes / `region_3d` members, `DecimateModifier.face_count` (needs per-modifier results in `EvaluatedObject`) and the DISPLACE image reader → M8; `Render Result`/`Viewer Node` images, `Light.node_tree` and image edits reaching renders → M5. Closed: `--verbosity full` `modifier_status` (M34S-7). New row: `Depsgraph` properties are not bound (M8). | deviations §2 |
| M34S-5 | `uv.smart_project` tie sensitivity | Documented in the §2 smart_project row: positions one ulp apart (SUBSURF at T2) reorder tied face areas whatever the face count; T3 golden `uv/smart/subsurf_cube.py`. The row's "M3 follow-up" becomes M8. | deviations §2 |
| M34S-6 | M3 exit count | `frap-golden/tests/golden.rs::m3_static_gltf_goldens_with_round_trips`: at least 60 first exports in `gltf/m3_workflows/` and at least as many `_rt` round trips (round trips are named after the export, its folder or a variant, and `modifier_stack_reimport.py` round-trips another case's export, so the suite is checked by count). | §22.5 |
| M34S-7 | Agent-facing protocol | (a) Unknown enum items (assignments, enum sets, operator arguments) carry `detail.did_you_mean` (closest identifiers, upper-cased comparison); the edit-distance helper moves to `frap_rna::error::suggest`, which `frap_op::registry::suggest` calls. (b) Pointers into an object's evaluated mesh are named by the evaluated expression `bpy.data.objects['X'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data…` in `path`, `$coll`, `$ptr` and `$func` (`frap_session::resolve::ptr_repr`; deviations §3). (c) A property of a `Depsgraph` pointer answers `RNA_NOT_IMPLEMENTED` naming `Depsgraph.<prop>` instead of `ViewLayer.depsgraph`. (d) Hint `FUNC_NOT_CALLED` on a statement that reads a function without calling it (§13.2). (e) `--verbosity full`: `context.evaluated` and `context.modifier_status` for the active object (§13.2; `frap-schema` `Context` gains both optional members). | §13.2, §13.4 |
| M34S-8 | Stale evaluated reads (found while testing M34S-7e) | `object.modifier_add`, `Object.modifiers.new`, `object.subdivision_set` (when it adds a modifier), `object.shade_auto_smooth` and Object Mode `mesh.separate` replaced the ID with a modified copy that still carried the generation from before `get_mut`/`object_mut` bumped it, so an evaluated object cached before them (read cache, session evaluator, renders) was served again: e.g. a second `modifier_add(type='SUBSURF')` after an evaluated read still read 26 vertices. Each site now keeps the bumped generation; regression test `frap-session/tests/m34_stabilization.rs`. | §5.3, §7.3 |
| M34S-9 | CLI transcripts and schema | Transcripts re-blessed for M34S-7 (`did_you_mean` on enum errors in `m1/vertical_slice`, `m1/object_ops/add_duplicate_join`, `m2/edit_mode/export_from_edit`, `m3/gltf/export_options`, `m3/materials/glass_bottle`; the evaluated repr in `m4/export_apply/linked_duplicates`); `frap-schema` `protocol.snap` gains the two `Context` members. | §20.1 |
| M34S-10 | Gate | `cargo build`/`test` (2053 tests)/`clippy --all-targets -D warnings`/`fmt --check`, `cargo xtask fmt --check`, `check-deps`, `codegen --check`, the CLI transcripts, `frap-cli` `blender_live`, and the golden suite against Blender 5.2.4 (`cargo xtask golden live`: 922 cases) are green. Remaining (not blocking): Workbench `VERTEX`/`TEXTURE` (M5), `DecimateModifier.face_count` and the other retargeted §2 rows (M34S-4). | §20.5 |
| M34S2-1 | `--verbosity full` never unwinds | `Session::evaluated_json` (the context built after a statement, still under its budget) runs `evaluated_for_read` inside `catch_unwind`: an evaluation past `--stmt-timeout` (or any other unwind) omits `context.evaluated` and reports every modifier `ERROR` with the message, instead of killing the process (exit 101, no output, also under `frap serve`). `Session::get/set/call` open the statement budget too. Regression tests: `frap-cli/tests/m4_cli.rs` (CLI and serve, SUBSURF level 11 over `--max-elements`, level 10 past `--stmt-timeout`). | §12, §13.2 |
| M34S2-2 | One limit policy for modifier evaluators | `frap_modifiers::stack::evaluate` catches an `ElementsExceeded` unwind per modifier (`eval_within_limits`): a result over `--max-elements` is the modifier's error (`frap: limit exceeded (max_elements): …`, WARNING, status `ERROR`, mesh passed through) whether the evaluator pre-checks (SUBSURF, ARRAY, BEVEL), returns the error (SCREW) or hits the BMesh backstop; `TIMEOUT` still ends the statement. `stack::is_limit_error`; `object.modifier_apply` and `object.convert` answer such an error with `LIMIT_EXCEEDED` (rollback) instead of `OP_NOT_IMPLEMENTED`. D-52 extended. | §12, D-52 |
| M34S2-3 | ID properties | `ID["key"]` reads and writes `IdHeader.custom` (`frap_session::resolve::idprops`; writes in `Session::set_path`, `Main::header_mut`), with Python's conversion rules (32-bit ints with `OverflowError` as `ValueError` + `detail.python_exception`, doubles, bools, strings, int/float arrays, bool/string/dict lists, groups; 63-byte keys; Blender's KeyError/TypeError texts), `+=`, nested reads (`ob["g"]["k"]`, `ob["a"][0]`), and the `bpy_struct` methods `keys()`, `values()`, `items()`, `get(key, default)` on IDs. Unsupported values (None, ID pointers, embedded IDs, element writes) answer `RNA_NOT_IMPLEMENTED` (deviations §2 row, M8). glTF `extras` authored in both tools: golden `gltf/attributes/extras_authored` (and `extras.py` now runs its property lines in both tools). | §9, WP 3.7 |
| M34S2-4 | Modifier object pointers (D-97 closed) | `frap_scene::id::object_pointer_rule` (ported from `rna_modifier.cc`: `PROP_ID_SELF_CHECK` on every modifier `Object` pointer but UVWarp's from/to, `RNA_MOD_OBJECT_SET` type filters), checked in `resolve::set_prop`: the modifier's own object raises Blender's `TypeError`, an object of another type is ignored (pointer unchanged) and the assignment echoes the pointer's value. Golden `modifiers/stack/pointer_rules`. | D-97 |
| M34S2-5 | glTF export details | (a) `export_scene.gltf(export_apply=True)` evaluates the meshes at frame 0 unless `export_current_frame=True` (`export.save`'s `frame_set(0)`, restored; the scene change is not logged); `export_current_frame` is a supported prop now. Golden `modifiers/integration/export_apply_frame0` (BUILD: 9 vs 10 faces). (b) A texture gathered from several sockets warns `More than one shader node tex image used for a texture. …` (`__gather_sampler`; also for one image node feeding metallic and roughness, verified). (c) `Image.pack()` of a generated image stores `packed_files[0].filepath == ''` (the buffer's path), so `unpack()` names the file after the image (golden `images/pack_generated_path`). | §17.2, WP 3.2, WP 3.5 |
| M34S2-6 | Agent UX and coverage | `object.modifier_apply/remove/copy/move_up/move_down/move_to_index` with a name no edited object has stay `CANCELLED` (Blender) and add a `frap:` WARNING naming the object(s) with `did_you_mean` or the existing modifier names. M4 CLI transcripts `tests/cli/m4/modelling/modifier_ops_{bpy,short}` (both syntaxes). Documented: `uv.lightmap_pack` qsort ties (deviations §2 qsort row, T3 golden `uv/lightmap/sphere_ties`), DECIMATE after SUBSURF (D-120, T3 golden `modifiers/bevel_decimate/decimate_after_subsurf`), vertex-group modifier settings waiting for M6 (deviations §2). `cargo xtask fmt` applied to the automod module `frap-material/src/patterns/pbr_specular_glossiness.rs`. | deviations §1/§2 |
| M34S2-7 | Gate | `cargo build`/`test` (2061 tests, 0 failures)/`clippy --all-targets -D warnings`/`fmt --check`, `cargo xtask fmt --check`, `check-deps`, every CLI transcript, and `cargo xtask golden live` against Blender 5.2.4 (928 cases, all passing). | §20 |

## Decision log (M3+M4 stabilization round 3)

Fixes for the M3+M4 exit verifier's third report, on `m3-m4-materials-modifiers`.

| # | Topic | Decision | Where |
|---|---|---|---|
| M34S3-1 | `subdiv-osd` M4 gate | frap's own Catmull-Clark stays the SUBSURF backend (`frap_subdiv::default_subdivider`), and `subdiv-osd` is on by default (`frap-subdiv` `default = ["osd"]`, also in `frap-cli`'s defaults; the boolean-manifold pattern). Where Blender evaluates points between isolation-level vertices inside Gregory end caps (`use_limit_surface` with `levels > quality`, or `SIMPLE` with `levels > 1`, whose all-sharp level-1 faces around the face point of a non-quad are irregular) the points come from the `opensubdiv-rs` patch table (`catmull_clark::end_caps`): SIMPLE level 2 on a cone was 0.127 off, now within 1e-6 (cone, cylinder, UV sphere, Suzanne, levels 2-3; Catmull-Clark level 4 on a cylinder and a UV sphere). `opensubdiv-rs` has no `generateLegacySharpCornerPatches`, so the isolation-level face at a smooth boundary corner keeps frap's own evaluation, which now sharpens such corners from the isolation level on (Blender's regular sharp-corner patch). UVs inside face-varying Gregory caps still differ (D-121, M8). Goldens `modifiers/subsurf/{simple_end_caps,simple_end_caps_monkey,levels_above_quality}.py` (T2); dumps `frap-subdiv/tests/data/{cone,cylinder,sphere}_l{2,3}_simple`, `tris_l3_simple`, `cylinder_l4`, `ngon_l4`. `deny.toml`'s TOST-1.0 exception for `opensubdiv-rs` now covers a default dependency. | §3.5, §3.6, §22.6 |
| M34S3-2 | UV sphere edge and face order (D-19) | `primitive_uv_sphere_add` replays `bmo_create_uvsphere_exec` (`frap-mesh/src/primitives/uv_sphere.rs::blender_order`): profile edges, each `extrude_edge_only` (duplicated edges in pool order, faces and connecting edges in the `GHash` order of 80-byte `BMEdge_OFlag`s in one contiguous chunk), then `weld_verts` (edges re-created in pool order and appended, faces rebuilt in place). Blender's order changes between runs, but DECIMATE was found to depend on it: with an order Blender produces, `COLLAPSE` 0.2/0.3/0.5, `DISSOLVE` (default) and `UNSUBDIV` match Blender (golden `modifiers/bevel_decimate/decimate_uv_sphere.py`, T2). `DISSOLVE` at 0.2/1.0 and SCREW (face winding follows edge orientation) vary between Blender runs themselves (D-19; SCREW at T3, `modifiers/b/screw/uv_sphere.py`, with the same counts). D-120 extended to DISSOLVE after SIMPLE SUBSURF (T3 golden `modifiers/bevel_decimate/dissolve_after_subsurf.py`). | §6.1, deviations D-19/D-120 |
| M34S3-3 | glTF `extras` with a non-finite float | `export_extras=True` with a NaN/inf custom property fails the whole export like Blender (the add-on's `json.dumps(allow_nan=False)` → `RuntimeError` naming `Out of range float values are not JSON compliant: nan`; nothing written); frap's message also names the property and the datablock (`extras::NonFinite`, `ExportError::Json`). Test `frap-ops-io-tests/tests/gltf_ops.rs::non_finite_extras_fail_the_export`. | §17.1 |
| M34S3-4 | Short-form `bpy.` values | A short-form value that starts with `bpy.` (`context.window.scene = bpy.data.scenes['S2']`) answers `SyntaxError` with the hint `write data.scenes['S2'] (not bpy.data.scenes['S2']), or write the whole statement in bpy form` instead of the unquoted-text hint (corpus `err_short_bpy_root_value*`). | §11.4, §13.2 |
| M34S3-5 | M3 deferrals recorded | Lossless WebP export (`EXT_texture_webp`) moves from M3 to M7 with the lossy encoder (deviations §2: WebP is written as PNG with a WARNING; import of `EXT_texture_webp` is M3 and done). `KHR_materials_variants` is not one of the "every KHR material extension" of the M3 Shippable: its data model is the add-on's (variant lists on the scene, per-slot mappings) and it stays M7 as §22.9 plans ("KHR_materials_variants complete"). OPERATORS §F rows updated. | §22.5, §22.9, OPERATORS §F |
| M34S3-6 | Modifier normals | Corner normals of SOLIDIFY/MIRROR bisect/BEVEL `VERTICES` stacks and WEIGHTED_NORMAL `CORNER_ANGLE` custom normals differ from Blender by float accumulation order (~2e-5, one INT16 unit): D-122, such cases compare at `tol=1e-4`. | deviations D-122 |
| M34S3-7 | Gate | `cargo build`/`test --workspace` (2063 tests, 0 failures)/`clippy --workspace --all-targets -D warnings`/`fmt --check`, `cargo xtask fmt --check`, `check-deps`, `codegen --check`, every CLI transcript, `frap-cli` `blender_live`, and `cargo xtask golden live` against Blender 5.2.4 (934 cases; the one live drift, Blender's own run-dependent SCREW of a UV sphere, became the T3 case of M34S3-2). Remaining (documented, not blocking): UVs inside face-varying Gregory caps (D-121, M8), modifier-stack normals at ~2e-5 (D-122). | §20 |

## Decision log (M3+M4 stabilization round 4)

Fixes for the M3+M4 exit verifier's fourth report, on `m3-m4-materials-modifiers` (after merging
`m2-edit-mode`'s CI fixes: integer-mask half-float quantisation for lavapipe, C `strerror` texts,
native paths in the mainfile operators).

| # | Topic | Decision | Where |
|---|---|---|---|
| M34S4-1 | `export_apply` with several scenes | The operator evaluates each exported scene (`scenes::exported_scenes`: every scene, or the window's with `use_active_scene`/a collection export) with the window switched to it, as the add-on's `VExportTree.construct` sets `bpy.context.window.scene`: that scene's view layer decides which objects its depsgraph evaluates (`apply::viewport_evaluated_objects`) and its own `frame_current` is the evaluation frame (`frame_set(0)` only moves the user's scene). An object linked into several scenes is evaluated in each scene (corrected by M34S5-1: the earlier claim that Blender's cross-scene reuse was non-deterministic did not reproduce). The window goes back to the user's scene. Golden `modifiers/integration/export_apply_scenes.py` (T2, from either window scene). | `frap-ops-io/src/ops/export_scene__gltf.rs`, `frap-io-gltf/src/export/scenes.rs` |
| M34S4-2 | DECIMATE on the UV sphere (D-19) | Blender's own COLLAPSE result on the default UV sphere changes between runs (the sphere's edge order comes from a pointer-keyed `GHash`; 20 runs of 5.2.4 gave 5 different meshes at ratio 0.2), so `decimate_uv_sphere` keeps only the run-independent DISSOLVE (default angle) and UNSUBDIV at T2, adds COLLAPSE 0.2/0.3/0.5 on ico spheres (identical in every run, many cost ties) at T2, and checks COLLAPSE of the UV sphere at T3 on the observed bands (`decimate_uv_sphere_collapse_02/03/05`: manifold, Euler 2, vertex count exact, edges/faces within one). | tests/golden, deviations D-19 |
| M34S4-3 | SOLIDIFY `NON_MANIFOLD` + SUBSURF normals (D-122) | Not a SOLIDIFY bug: on a cube and a plane the stack matches Blender to 1e-6. After a bisecting MIRROR, Suzanne's positions differ by float rounding (~1e-7) and SUBSURF's limit normals amplify it at near-degenerate corners (max 3.1e-4 on one face of ~15k corners). D-122 now states the measured bound; golden `modifiers/a/solidify/nonmanifold_subsurf.py` at `tol=4e-4`. Bit-exact accumulation order stays M8 (with D-71). | deviations D-122 |
| M34S4-4 | glTF re-import normals (D-119) | Re-measured (one cycle only; a second cycle of identical input still differed, fixed by M34S5-3): re-exporting a Blender-written file after import is bit-identical to Blender for every `export_apply` and M3 workflow fixture (equal vertex and index counts, equal normals); frap's own export→import→export differs by one INT16 step (≤ 1.0e-4), two cycles by ≤ 2.0e-4, with the same vertex split. No vertex-count difference was reproducible; D-119 states the bounds per cycle. | deviations D-119 |
| M34S4-5 | `extensionsUsed` order | The add-on declares extensions in its exporter traversal order (scenes, then depth first in `dir()` order: a node's children before its mesh; per material `emissive_texture`, then the extensions dict, then the other textures), and every non-unlit material carries possibly-empty clearcoat, transmission, emissive-strength, specular, anisotropy and IOR objects in `__gather_extensions` order, whose declarations survive when any material keeps them. `export/ext_order.rs` reorders frap's first-use list by that traversal of the finished document (verified on 5.2.4 with three import orders: always `[clearcoat, transmission, sheen]`). Approximation: every non-unlit material counts as Principled with unlinked Anisotropic/IOR (otherwise the add-on skips specular/anisotropy/IOR objects), which only affects the relative order of extensions a later material keeps. `gltf_canonical` now compares `extensionsUsed`/`extensionsRequired` in file order (every existing fixture still passes); golden `gltf/materials_ext/declaration_order.py`. | `frap-io-gltf/src/export/ext_order.rs`, `frap-golden/src/compare/gltf_canonical.rs` |
| M34S4-6 | `GLTF_EMBEDDED` discoverability | A dynamic-enum item the context does not offer can register a hint (`frap_op::DYN_ENUM_HINTS`): `export_scene.gltf(export_format='GLTF_EMBEDDED')` without the preference keeps Blender's `TypeError: … enum "GLTF_EMBEDDED" not found in ('GLB', 'GLTF_SEPARATE')` and gets `hint` = run `bpy.context.preferences.addons['io_scene_gltf2'].preferences.allow_embedded_format = True` first (already settable). | `frap-op/src/def.rs`, `def/pipeline.rs` |
| M34S4-7 | Windows parity of M3/M4 file I/O | OS errors of image load/save (`Image.load`/`images.load`/`image.open`, `Image.save`/`save_render`), glTF import (`Couldn't read …`), glTF export (shared `write_files`) and render output (`Render error (…) cannot save`) read as C `strerror` through `frap_rna::error::strerror`, as Blender's reports (also on Windows, where the MSVC CRT maps Win32 errors to `errno`). Paths in these messages stay as given (Blender prints the operator's `filepath`, not a canonicalised one). `frap-imbuf::load/save` keep Rust's text: a leaf crate without `frap-rna`, whose errors no operator shows (callers use `.ok()`). New goldens use relative `out/…` paths only (separator-agnostic). | `frap-scene/src/id/image.rs`, `frap-io-gltf/src/import/mod.rs`, `frap-ops-render/src/output.rs` |
| M34S4-8 | Gate | `cargo build`/`test --workspace --no-fail-fast` (2066 tests, 0 failures)/`clippy --workspace --all-targets -D warnings`/`fmt --check`, `cargo xtask fmt --check`, `check-deps`, `codegen --check`, every CLI transcript, and `cargo xtask golden live` against Blender 5.2.4 three times in a row, green each time (940 cases). Remaining (documented, not blocking): UVs inside face-varying Gregory caps (D-121, M8), modifier-stack normal rounding (D-122, M8), re-import normal rounding (D-119). | §20 |

## Decision log (M3+M4 stabilization round 5)

Fixes for the M3+M4 exit verifier's fifth report, on `m3-m4-materials-modifiers`.

| # | Topic | Decision | Where |
|---|---|---|---|
| M34S5-1 | `export_apply` of an object linked into several scenes (corrects M34S4-1) | The add-on keeps `bpy.context.window.scene` on the scene it gathers, so `__gather_mesh`'s `evaluated_depsgraph_get()` is that scene's depsgraph, and the object's temporary mesh (part of the cache key) is that depsgraph's: every scene's node gets its own evaluation and its own glTF mesh (verified on 5.2.4, identical in 10 of 10 runs: a Cube and Suzanne with a Build, linked into scenes at frames 9 and 13, export 16/1378, 20/1770 and 4/484 vertices per scene; a Subdivision cube in three scenes exports three glTF meshes sharing only the index accessor). `ExportSettings::evaluated_meshes` is keyed by (scene, object) (`EvaluatedMeshes`), the operator evaluates every exported scene's objects in that scene's depsgraph, `Gather::scene` is the scene `scenes::gather` is gathering, and `apply::evaluated` / the applied-mesh cache read and key by it. Before, the first scene's evaluation was reused, so a node of the window scene whose Build had not started yet lost the geometry of the other scene (and vice versa). Goldens `modifiers/integration/export_apply_shared_scenes.py` (two Build objects in three scenes at frames 1/9/13, from each window scene) and `export_apply_shared_window_scene.py` (T2). | `frap-ops-io/src/ops/export_scene__gltf.rs`, `frap-io-gltf/src/export/{settings,gather,scenes,apply}.rs`, `export/mesh/gather.rs` |
| M34S5-2 | `extensionsUsed` order with dispersion | `export_dispersion` returns a (possibly empty) dispersion object for every material that has a volume and a glTF Material Output group with a Dispersion socket, between iridescence and IOR in `__gather_extensions`; `ext_order.rs` now declares dispersion for every material with `KHR_materials_volume` (approximation: the group is assumed to have the socket, as the importer's and the add-on's own groups do). Volume, sheen, dispersion materials in that order declare `[transmission, volume, dispersion, sheen]` (verified on 5.2.4, also with clearcoat and iridescence imported first). Golden `gltf/materials_ext/declaration_order_dispersion.py` (both object orders and an import/re-export). | `frap-io-gltf/src/export/ext_order.rs` |
| M34S5-3 | glTF re-import normals (D-119) | Root cause found: `mesh_set_custom_normals` normalises the custom normals (`math::normalize`, a division by the length) before encoding them; frap encoded the raw glTF vectors. A normal whose squared length rounds to 0.99999994 then has a dot product with its fan normal of 0.9999999 instead of exactly 1, and `alpha` encodes as 7 instead of 0 (Blender's `(0, -16384)` on the top-centre corner of the stack fixture's cube; 2e-4 after decoding). The importer now normalises before encoding (zero vectors still take the vertex normal): importing Blender's re-export of the `export_apply_stack`, `boolean` and `auto_smooth` fixtures and exporting again is identical to Blender, index by index, for every node. frap's own export→import→export keeps the D-119 bound (the first export's positions/normals are within float rounding, which can move an encoded value one INT16 step). Unit test `custom_normals_are_normalized_before_encoding` (Blender's values). | `frap-io-gltf/src/import/mesh/custom_normals.rs`, deviations D-119 |
| M34S5-4 | WIREFRAME after SUBSURF (D-122) | Not a WIREFRAME bug: WIREFRAME alone matches Blender at T1, and so does WIREFRAME + DISPLACE on the torus. SUBSURF's Catmull-Clark positions are within float rounding (~1e-7, OpenSubdiv's accumulation order, as D-120 states) and the 0.02-thick wireframe faces amplify it in their corner normals (default torus: 812 corners above 1e-5, at most 3.2e-5 on one face); a DISPLACE after them moves the vertices along those normals and the thin faces amplify again (corner normals ≤ 1e-4 evaluated, ≤ 1.5e-4 in the `export_apply` glTF). D-122 lists the stack with its bounds; goldens `modifiers/b/wireframe/after_subsurf.py` (tol=5e-5) and `after_subsurf_displace.py` (evaluated and glTF, tol=2e-4). Bit-exact Catmull-Clark accumulation order: M8 (with D-71). | deviations D-122 |
| M34S5-5 | Gate | `cargo build`/`test --workspace --no-fail-fast` (2068 tests, 0 failures)/`clippy --workspace --all-targets -D warnings`/`fmt --check`, `cargo xtask fmt --check`, `check-deps`, `codegen --check`, and `cargo xtask golden live` against Blender 5.2.4 three times in a row, green each time (945 cases). Found while investigating M34S5-3, not fixed this round: `MeshPolygon.normal` on a mesh with INT16 custom normals reads frap's `Mesh::face_normals()` (mixed from the corner normals, which `Mesh.polygon_normals` rightly returns), whereas Blender's RNA getter computes the true face normal (`face_normal_calc`; up to 0.076 apart on the imported stack cube); target M5. | §20 |


## Decision log (M5+M6 W0, WPs 5.0 + 6.0: merged contract delta and stubs)

M5 and M6 run in parallel (§22.8 WP 6.0's note, §22.11), so one integrator did WP 5.0 and WP 6.0 together on branch `m5-m6-render-animation`, from the M3+M4 stabilization round 5 head. It first closed the M3+M4 leftovers found by verification (M56W0-5), then applied both contract deltas. Every change is additive unless stated otherwise.

| # | Topic | Decision | Where |
|---|---|---|---|
| M56W0-1 | Codegen | `spec/current_milestone` = 6 (M5 has no operator): typed props for the 54 M6 operators — `frap-spec-ops-anim/src/generated/{anim,armature,nla,pose,screen}.rs` (32), `object` +20, `mesh` +2 (288 typed in all). Drift tests: `anim` 32, `object` 85, `mesh` 120, catalog 288 (its dynamic-enum example is now `object.constraint_add`, `vertex_group_set_active` being typed); `xtask` codegen test pinned at 6. `codegen --check` clean. Hand-written M6 data types (no generated plain structs): their defaults are `armatures.new`/`actions.new`/`keyframe_points.insert`'s, checked by WPs 6.1-6.3 against dumps. | §4.2 |
| M56W0-2 | Merged wave plan | W1 = 5.1, 5.2, 5.3, 5.4, 6.1, 6.2, 6.3; W2 = 5.5, 5.6, 6.4, 6.5, 6.6, **6.10** (new: vertex-group options of the M4 modifiers, which deviations §2 listed for M6 without an owner); W3 = 6.7, 6.8, 6.9. No Needs column forced another change. Golden prerequisite sets in §22.8 "Merged plan": the Action data API moves to 6.1 (W1), so 6.4's goldens (W2) and 6.1's own can build curves. | §22.7, §22.8 |
| M56W0-3 | Ownership overlaps | Resolved so no two WPs of one wave own a path: `render__render.rs` is 6.6's (W2); 5.4 owns `output.rs` + `output/**` and `frap-imbuf/src/encode/**`, and the engine choice moved out of the render op into the service (`supports_engine`, M56W0-6), so 5.1 never edits the op; `frap-ops-object/src/ops/` by file (6.2 `armature_add`, `posemode_toggle`; 6.3 `vertex_group_*`, `vertex_weight_*`, `shape_key_*`, `join_shapes`, `modifier_apply_as_shapekey`, `join.rs` + `join/**`; 6.5 `parent_set.rs`) and `modes/` (6.2 `edit_armature`, `pose`; 6.3 `edit_mesh`, reassigned for shape layers); `frap-scene` ids (`armature*.rs` 6.2, `key.rs` 6.3) and `hooks/` by prefix (6.1 `object_animation_data*`/`id_animation_data*`/`anim_data_*`/`action_*`; 6.2 `armature_*`/`bone_*`/`edit_bone_*`/`pose_*`/`object_pose*`; 6.3 `object_vertex_groups*`/`vertex_group_*`/`object_shape_key*`/`shape_key_*`/`key_*`/`mesh_shape_keys*`); `frap-ops-data/src/funcs/` by the real stems (6.1 `blend_data_actions_*`; 6.2 `blend_data_armatures_*`, `armature_edit_bones_*`, `armature_bones_*`; 6.3 `object_vertex_groups_*`, `vertex_groups_*`, `vertex_group_*`, `object_shape_key_*`, `shape_key_*`, `mesh_transform` — the plan's `shape_key_*` for `Object.shape_key_add` is `object_shape_key_add`); `frap-ops-anim/src/funcs/` by prefix (6.1 the data API, 6.4 the rest in W2); `frap-modifiers/src/types/` (6.5 `armature.rs`; 6.10 the vertex-group modifiers); `frap-eval` (`pose.rs` 6.2, `key_eval.rs` + `cache/fingerprint.rs` 6.3, `animate.rs` 6.4); glTF: 6.8 owns `export/{nodes,vtree}.rs` and `import/{nodes,vnode,mesh}.rs` (bones are nodes), 6.7 and 6.9 read the shared `export::node_map::NodeMap` / `VNode::blender_bone_name`; `export/mesh/{writers,post}/` are per-concept files (6.8 `skin.rs`, `morph_targets.rs`; 7.1's meshopt/quantize come in M7). Golden folders: 5.1 `render_eevee/core`, 5.2 `render_eevee/world`, 5.5 `render_eevee/features`, 5.6 `render_eevee/scenes`, 5.4 `render/output`, 6.1 `anim_eval`, 6.4 `anim`, 6.2 `armature` except `armature/deform` (6.5). | §22.7, §22.8 |
| M56W0-4 | Gaps without an owner | Assigned (deviations §2 rows updated): EEVEE rendering 5.1; Workbench `VERTEX`/`TEXTURE`, `Image::acquire` in render builds and the contact sheet's warnings 5.5 (`workbench/**`, `build/{images,mesh}.rs`, `frap__render_contact_sheet.rs` reassigned); the factory `Render Result`/`Viewer Node` images and `Light.node_tree` retargeted to M8 (no M5 WP renders through them); `.frap` persistence of actions, NLA, armatures, poses, shape keys and vertex groups 6.4 (`frap-project/src/scene_map/**` reassigned); `object.join`/`mesh.separate`/`Mesh.transform` with shape keys 6.3; `deformed_by` with virtual modifiers 6.5 (`frap-ops-object/src/support/deform.rs`); vertex-group modifier options 6.10; glTF animation export props 6.7. Rule 1: `mesh.blend_from_shape`, `mesh.shape_propagate_to_all`, `object.join_shapes`, `object.modifier_apply_as_shapekey` → 6.3; `anim.*`, `screen.*`, `nla.*` → 6.4; `armature.*`, `pose.*` → 6.2 (OPERATORS.md notes). | deviations §2, OPERATORS.md |
| M56W0-5 | M3+M4 leftovers | (1) **Not fixed, root cause found (D-124)**: the glTF vertex counts of subdivided UV meshes (Suzanne + SUBSURF 1: Blender 2893, frap 2109; the MIRROR+SUBSURF 2+SOLIDIFY+smart_project stack: 41666 vs 39144) come from OpenSubdiv's per-ptex-face face-varying evaluation, whose float results for corners sharing a vertex differ by 1-2 ulps; the exporter splits them (exact `np.unique`). Verified with a C++ program linked against Blender's own `libosdCPU.dylib`: fed frap's topology descriptor and ptex coordinates it reproduces Blender's UVs bit for bit on five meshes; frap's backend matches 9% of corners bit-exactly, the `opensubdiv-rs` backend 20%. Matching needs a bit-exact port of the face-varying pipeline with the compiler's FMA contractions: §22.10 8.11. (2) `MeshPolygon.normal` (item and `foreach_get`) is `face_normal_calc` of the positions (`frap-mesh` `normals::rna_face_normal`): bit-identical to Blender after importing the `export_apply_stack` file (D-123 closed); the golden dumps gained an optional `face_normals` (`polygons/normal`, compared when both sides have it, so older fixtures still pass), golden `modifiers/integration/reimport_polygon_normals`. (3) ARRAY `use_merge_vertices`: `dm_mvert_map_doubles` sorts with the C library `qsort` (macOS libc order, `frap_bmesh` `c_qsort`, D-67), which decides which coincident twin a source maps to; golden `modifiers/a/array/merge_coincident_input`. (4) `extensionsUsed`: `export_iridescence` creates a (possibly empty) object for every Principled material with the glTF Material Output group's iridescence sockets; `materials::gather` records such objects per glTF material (`ext_order::CreatedEmpty`, also for dispersion, replacing M34S5-2's approximation when the record exists); golden `gltf/materials_ext/declaration_order_iridescence`. (5) BEVEL / `mesh.bevel` `harden_normals` ported (`bevel_harden_normals`, `bevel_edges_sharp_boundary`; D-79 closed, no WARNING any more); the BMesh custom-normal code moved from `frap-ops-mesh/src/ops/flip_normals/clnor.rs` to `frap_bmesh::clnor` (re-exported); goldens `edit/bevel/harden_normals`, `modifiers/bevel_decimate/bevel_harden_normals` (T1, tol 1e-4). | deviations D-79, D-123, D-124 |
| M56W0-6 | M5 contract (§19.2, additive) | `RenderService::supports_engine(engine)` (default: Workbench only); `frap-render`'s `Renderer` answers `BLENDER_EEVEE` with `eevee::pipeline::IMPLEMENTED` (false until 5.1) and dispatches `scene.settings.engine`; `render.render` and `frap.render_contact_sheet` ask it (`effective_engine`: `CYCLES` → `BLENDER_EEVEE` once supported, D-12; the Workbench-effect warnings only when Workbench renders). `frap-render/src/eevee/mod.rs`: `Eevee` (an `Engine`), `EeveeFeature` + `EEVEE_FEATURES` (WGSL snippet, `applies`, `pack` into `pipeline::MaterialParams`), `features()`, the `features/` automod directory, stubs `pipeline` (`IMPLEMENTED`, `MaterialParams`, `State`, `render`), `lights`, `shadows`, `taa`, `film`, `ibl::{WorldLighting, world_lighting}` (5.1 lights a constant world itself). `frap-render-scene`: `RSettings::eevee: REeveeSettings` (shadow ray/step counts, resolution scale, light threshold, clamps, ray tracing, fast GI, `filter_size`, `dither_intensity`), `RLight::eevee: RLightEevee` (temperature, per-component factors, soft falloff, normalize, cutoff, shadow filter/resolution/jitter, spread), `RenderScene::image_ids` + `image(IdRef<ImageId>)`, `RMesh::{uv_names, uv_active_render, color_names}`; the builder fills them. `frap-color`: `agx` (the M1 LUT entry moved from `lut.rs`), `filmic`, `pbr_neutral`, `aces`, `false_color` (each a `BAKED: &[BakedLut]` list `lut::baked` chains), `looks`. `frap-imbuf/src/encode/{png,jpeg,webp,exr,hdr,raw8}.rs`: the M1 encoders moved out of `formats.rs` unchanged (`formats::encode` checks, then delegates). | §19.2, §19.5 |
| M56W0-7 | M6 contract (§18, additive) | `frap-anim`: `action` (`Action { slots, layers, last_slot_handle, frame range, use_cyclic }`, `ActionSlot`, `ActionLayer`, `ActionStrip::Keyframe(KeyframeStrip)`, `Channelbag`, `ActionGroup`, `Action::channelbag`), `fcurve` (`FCurve`, `Keyframe` with Blender's defaults, `Interpolation`/`Easing`/`HandleType`/`KeyframeType`/`Extrapolation` with RNA identifiers), `anim_data` (`AnimData` — the M1 fields kept, `Default` = `animation_data_create()`: `use_nla`, influence 1, `REPLACE`, `HOLD` — `NlaTrack`, `NlaStrip`), `eval` (`evaluate_fcurve`, `recalc_handles`, `evaluate_action`, `evaluate_anim` over `ActionLookup` → `AnimChannel`s; `todo!()` for 6.1), `rna` (the `Action` binding, `todo!()` as before). `frap-scene`: `Armature { bones, collections, display_type, pose_position, … }`, `Bone`, `BoneCollection`, `Pose { bones }`, `PoseBone::new` (QUATERNION), `Key { user, key_blocks, use_relative, eval_time, animation_data }`, `ShapeKey`, `edit_armature::{EditArmature, EditBone}` (moved out of `context.rs`); only `ID.name` is bound until 6.2/6.3. `frap-eval`: `pose::evaluate` (None until 6.2) fills `EvaluatedObject::pose`; `key_eval::apply` runs before the modifier stack (the base mesh unchanged without a key); `GeoCx::pose` reads the armature's evaluated pose (ARMATURE modifier, 6.5). `frap-bmesh/src/shape.rs` stub. `frap-ops-anim`: crate skeleton (`keying.rs`, automod `ops/` and `funcs/`, dependencies). `frap-ops-transform/src/pose/mod.rs` stub. | §18 |
| M56W0-8 | glTF extension points | Export: `export::node_map::NodeMap` (`(scene, object)` and `(scene, armature, bone)` → node; `nodes.rs` records every object node), `animation::gather` called after the scenes (no-op), `skins.rs`, `pointer/` stubs. Import: `animation::{IMPLEMENTED, create}` (called after the scenes when implemented), `skins::IMPLEMENTED`, `morph::IMPLEMENTED` gate the M1 WARNINGs, `pointer/` stub, `VNode::blender_bone_name`. M3/M4 output unchanged. | §17.2 |
| M56W0-9 | Golden harness | `GMesh::face_normals` (optional, `props.toml` `face_normals = "polygons/normal"`, both dumpers; compared by `mesh_exact`/`mesh_canonical` when both sides have it); `compare=anim:<action>` dumps the action on both sides (`dump_state.py::dump_action` over every channelbag; frap natively): slots, F-curves with extrapolation, keys (`co`, interpolation, handles) and the value sampled at every whole and half frame from 3 before the first key to 3 after the last (`evaluate_fcurve`, so `anim:` cases need 6.1). | §20.2, §20.3 |
| M56W0-10 | Dependencies | `frap-render`: + frap-material, linkme, automod, half (frap-material leaves its dev-dependencies). `frap-ops-anim`: frap-op, frap-rna, frap-scene, frap-spec-rna, frap-spec-ops-anim, frap-anim, frap-eval, frap-mesh, frap-math, glam, libm, linkme, automod, smallvec, smol_str, rustc-hash, indexmap (dev: proptest, insta, approx, serde_json). `frap-golden`: + frap-geom, frap-anim. `frap-ops-all` dev: + frap-color. `check-deps` passes. | §3.5 |
| M56W0-11 | Contract pins | `crates/frap-ops-all/tests/contracts.rs` pins the M6 animation model and evaluation signatures, the armature/pose/key/edit-bone types and `EvaluatedObject::pose`, `supports_engine`, `REeveeSettings`/`RLightEevee`/`image_ids`/`image`, `Eevee`/`EEVEE_FEATURES`/`EeveeFeature::pack`/`pipeline::IMPLEMENTED`/`ibl::world_lighting`, `frap_color::lut::BakedLut`, `NodeMap` and `export::animation::gather`, and checks their defaults. | §3.4 |
| M56W0-12 | Bone context and selection (Blender 5.2 RNA) | `Context::{active_bone, active_edit_bone, active_pose_bone, selected_pose_bones, pose_object}` implemented (no `todo!()` left in `context.rs`): `active_bone` is `Armature.bones.active` outside Edit Mode and `active_edit_bone(&EditState)` inside it (a `Context` borrows no edit state, F-OP-5); `active_pose_bone`/`selected_pose_bones` follow `BKE_pose_channel_active_if_bonecoll_visible` / `PBONE_VISIBLE` with `bone_in_visible_collection`. Data model per Blender 5.2's RNA (dumped): pose selection and pose hiding live on `PoseBone` (`select`, `hide`), `Bone` has `hide_select` and no RNA `select` (its `select` triple stays as the `BONE_SELECTED`/`ROOTSEL`/`TIPSEL` storage across Edit Mode toggles), `Armature::active_bone` (`bones.active`), `EditBone::{hide_select, lock, collections}`. `context/rna.rs` keeps answering `None`/empty for these members until 6.2 (W1) binds the bone structs; 6.4 (W2) then takes the animation members. | §8, §22.8 |
