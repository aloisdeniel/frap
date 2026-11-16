# frap architecture proposal: FIDELITY-FIRST

Status: proposal for review. Date: 2026-11-16. Target: Blender 5.2 LTS Python API (checked against local Blender 5.2.4, build `cd5dc428cce1`).
Inputs: `docs/research/blender-ops-object.md`, `blender-ops-mesh-uv.md`, `blender-rna-datamodel.md`, `gltf-and-blender-io.md`, `rust-ecosystem.md`. This document cites them as [OBJ], [MESH], [RNA], [GLTF], [RUST].

This proposal takes one position: **Blender is the specification.** Every name, default, enum item, error message, selection side effect, element order and evaluation timing is copied from Blender 5.2. Where frap has to differ (headless-only ops, the missing Python interpreter, rendering on the GPU), it adds information or namespaced extras. It never changes Blender semantics to be "nicer".

---

## Table of contents

0. Principles, fidelity tiers and decisions needing sign-off
1. Cargo workspace and crate layout
2. Core scene data model (DNA): Main, IDs, naming, users
3. Mesh representation: Mesh (SoA attributes) vs BMesh (edit) vs evaluated Mesh
4. Depsgraph and modifier evaluation pipeline
5. Context, selection and mode model
6. Operator registry (wm)
7. RNA: typed property paths, collections, functions
8. Front-ends: bpy-line parser, short-form parser, value grammar, IR
9. JSON response protocol
10. Schema export (`frap --describe`)
11. `.frap` project format
12. glTF/GLB: model, exporter, importer, codecs
13. Renderer: wgpu, EEVEE-like, Workbench, contact sheet
14. Golden test harness against Blender
15. Error handling and transactional rollback
16. Determinism
17. Milestone plan with work packages and file ownership
18. Risks
Appendix A: verification probes run for this proposal

---

## 0. Principles, fidelity tiers and decisions needing sign-off

### 0.1 Principles

1. **Spec data comes from Blender, never from memory.** A set of committed Python scripts (`tools/blender/dump_*.py`) dumps operator RNA, struct RNA, node socket tables, modifier defaults, the factory startup scene and view-transform LUTs from `$BLENDER_BIN`. The JSON goes under `spec/blender-5.2/`. A Rust codegen step (`cargo xtask codegen`) turns it into static tables. Nobody hand-types an operator property, a default or an enum item.
2. **Mirror Blender's internal architecture at a smaller scale.** frap has a `Main` with ID datablocks (DNA), an RNA layer, a `wmOperator`-like registry with poll/exec and `is_set` flags, a BMesh with Blender's memory-pool ordering, a depsgraph with Blender's evaluation timing, and a modifier stack with Blender's `ModifierTypeInfo` split. Matching the structure is the cheapest way to match behaviour. A port of a Blender function then has an obvious place to live.
3. **Implement at the same level as Blender.** If Blender implements an operation on BMesh (bevel, triangulate, decimate), frap does too. If Blender works on the flat Mesh (array, mirror, solidify, weld, subsurf), frap does too. Element order follows from that choice.
4. **Errors are Blender's errors.** The Python exception class (`TypeError`, `RuntimeError`, `KeyError`, `ValueError`, `AttributeError`) and the message text are reproduced. Golden tests compare them line by line.
5. **Extras are namespaced.** frap-only operators live under `frap.*`, and frap-only operator properties start with `frap_`. frap-only response data (hints, change sets, mesh counts) sits in the JSON envelope, outside Blender values.
6. **Measure fidelity continuously.** Every implemented operator declares a fidelity tier (0.2), and the golden harness enforces it against real Blender output.

### 0.2 Fidelity tiers

| Tier | Meaning | Examples |
|---|---|---|
| **T0 surface** | Names, props, types, defaults, enum items, poll results and error class/message match Blender. Required for every registered op and RNA prop, implemented or not. | all |
| **T1 index-exact** | Element counts, element **index order**, positions (tol 1e-5), selection and active state match Blender index by index. | primitives, extrude*, inset, subdivide (uniform), split, poke, quads_convert_to_tris, flip_normals, merge, delete, object ops |
| **T2 canonical** | Order-independent equality: a sorted position multiset, rotation-normalised face cycles and per-corner attributes keyed by (face, vertex position). | bevel segments>1, dissolve_limited, tris_convert_to_quads, subsurf, boolean MANIFOLD |
| **T3 invariant** | Only invariants are checked: UVs in [0,1], no overlaps, area ratios, manifoldness, counts within a band. | uv.pack_islands layout, smart_project packing, boolean EXACT, decimate COLLAPSE |
| **T4 perceptual** | Image comparison with SSIM/RMS thresholds. | render.render EEVEE and Workbench |

The tier is data: `spec/frap-overlay/ops.toml` assigns it per operator. Ops start at T2 or T3 and get promoted to T1 when the golden suite proves index parity.

### 0.3 Decisions needing user sign-off

The fidelity angle forces some decisions the brief left open. Each has a recommendation.

| # | Decision | Fidelity-first recommendation | Why |
|---|---|---|---|
| D1 | **frap license** | **GPL-2.0-or-later.** | Index-exact parity for triangulation (`BLI_polyfill_2d` + beautify), bevel (`bmesh_bevel.cc`), subdivide (`bmo_subdivide.cc`), decimate, the UV parametrizer (ABF++/LSCM/SLIM), the UV packer, the subsurf mesh layout, EEVEE's light units and Suzanne's vertex table all need a port of Blender's GPL code. A clean-room MIT/Apache rewrite caps most of these ops at T2/T3. The glTF add-on (`io_scene_gltf2`) is Apache-2.0, so the I/O port is unaffected either way. If the user picks MIT/Apache, the plan holds but the T1 list shrinks and Suzanne must be dropped or remodelled. |
| D2 | Initial scene | **Factory startup** (Camera, Cube, Light, as with `blender -b --factory-startup`), plus `frap --empty` for `wm.read_homefile(use_empty=True)`. | Golden scripts and LLM training data both assume the default cube. |
| D3 | Reading `mesh.vertices` etc. in Edit Mode | **Reproduce Blender**: the data is stale until `mode_set('OBJECT')` or `obj.update_from_editmode()`. The response gets a `hints[]` entry naming the fix. | Same values as Blender, and the agent is told why. |
| D4 | Derived values (`matrix_world`, `dimensions`) after a property set | **Reproduce Blender's evaluation timing** (verified, Appendix A): stale until the next `bpy.ops` call, `view_layer.update()`, `evaluated_depsgraph_get()` or `frame_set`. A hint is attached when a stale derived value is read. | Verified Blender behaviour. A golden test catches any divergence. |
| D5 | Element index parity | **Emulate BLI_mempool LIFO slot reuse** and Blender's per-op create/kill order. | Needed so that `vertices[5].co` means the same vertex in frap and Blender. |
| D6 | `render.engine = 'CYCLES'` | Accept the value, since it is valid RNA. `render.render` renders with the EEVEE-like engine and emits a WARNING report: `Engine 'CYCLES' is not available in frap; rendered with BLENDER_EEVEE`. | Scripts written for Blender still run. |
| D7 | `primitive_monkey_add` | Depends on D1. With GPL, port the table. Otherwise return `NotImplementedError` with a message. | License. |
| D8 | Float printing | Shortest round-trip **f32** (`0.1`). The full Python-style double repr is available under `--py-repr`. | Blender stores f32. [RUST §8] |
| D9 | Variables, loops | Not supported (fixed by the user). Statements that assign to bare names give `SyntaxError: frap: variables are not supported; use a full bpy path`. | |

---

## 1. Cargo workspace and crate layout

The crates follow Blender's source tree names, so ported code has an obvious home: `makesdna` → `frap-dna`, `makesrna` → `frap-rna`, `windowmanager` → `frap-wm`, `bmesh` → `frap-bmesh`, `editors/*` → `frap-ed-*`, `depsgraph` → `frap-depsgraph`, `imbuf` → `frap-imbuf`.

```
frap/
├─ Cargo.toml                      # [workspace], [workspace.dependencies] pinned per [RUST §13]
├─ rust-toolchain.toml             # 1.97
├─ spec/
│  ├─ blender-5.2/                 # COMMITTED Blender dumps (source of truth)
│  │  ├─ ops.json                  # every operator: props, flags, defaults, enum items, bl_options, poll matrix
│  │  ├─ rna.json                  # bpy.types structs: props, funcs (params, keyword-only flags), subtypes
│  │  ├─ nodes.json                # shader node types: sockets (name, identifier, type, default, enabled rules)
│  │  ├─ modifiers.json            # fresh modifier defaults (modifiers.new values, not RNA defaults)
│  │  ├─ factory_startup.json      # full factory scene incl. RegionView3D of the Layout viewport
│  │  ├─ primitives/*.json         # reference vertex/face tables per primitive (test fixtures)
│  │  ├─ messages.json             # captured error/report strings for templating
│  │  └─ colormanagement/*.cube    # 3D LUTs: AgX, Standard, Filmic, Khronos PBR Neutral, False Color (dumped)
│  └─ frap-overlay/                # hand-written facts Blender cannot dump
│     ├─ ops.toml                  # status, fidelity tier, headless notes, frap_ props, dynamic-enum rules
│     ├─ python_only.toml          # children, users_collection, from_pydata, shade_smooth, keyframe_insert sig
│     └─ engines.toml              # hard-coded dynamic enums (render.engine, studio lights…)
├─ tools/blender/                  # Python run INSIDE Blender (dumps + golden runner)
│  ├─ dump_ops.py dump_rna.py dump_nodes.py dump_modifiers.py dump_factory.py dump_luts.py
│  ├─ golden_runner.py snapshot.py emulations.py
├─ xtask/                          # cargo xtask: codegen | spec-refresh | golden [--bless] | describe-diff
├─ crates/
│  ├─ frap-spec/        generated static tables (OpDef, StructDef, PropDef, EnumDef, NodeTypeDef)
│  ├─ frap-math/        mathutils semantics: Euler orders, compatible euler, quat (w first), mat decompose
│  ├─ frap-dna/         Main, ID types, Object/Mesh/Material/…, naming, user counts, factory startup
│  ├─ frap-geom/        polyfill2d+beautify, looptris, face/vert/corner normals, kdtree, BLI_rng, mikktspace
│  ├─ frap-bmesh/       BMesh, mempool, Euler ops, bm<->me, select flush, bmesh operators (bmo_*)
│  ├─ frap-anim/        F-curve eval, Bézier handle calc, keyframe insert, slotted actions, NLA eval
│  ├─ frap-modifiers/   ModifierTypeInfo + one module per modifier type
│  ├─ frap-depsgraph/   tags, relations, evaluation order, evaluated copies, copy-back, frame_set
│  ├─ frap-uv/          parametrizer (LSCM, ABF++, SLIM), smart project, projections, packer
│  ├─ frap-rna/         runtime RNA: pointers, path resolution, get/set/call, update tags, rna_*.rs
│  ├─ frap-wm/          Session, WindowManager, Context, operator registry, props, macros, reports, transactions
│  ├─ frap-ed-object/   object.*, collection.*, material.*, image.*, world.*, view3d.snap_*, scene.*
│  ├─ frap-ed-transform/transform.* (object, edit, pose), pivot, orientations, proportional
│  ├─ frap-ed-mesh/     mesh.* (primitives + edit ops), geometry.*
│  ├─ frap-ed-uv/       uv.*
│  ├─ frap-ed-anim/     anim.*, screen.frame_*, armature.*, pose.*
│  ├─ frap-imbuf/       image decode/encode, colorspace, packed files, channel packing
│  ├─ frap-gltf/        glTF 2.0 serde model, GLB container, accessor codec, extension codecs
│  ├─ frap-io-gltf/     import_scene.gltf / export_scene.gltf (port of io_scene_gltf2 5.2.41)
│  ├─ frap-project/     .frap zip container, versioning, wm.open/save_mainfile backends
│  ├─ frap-render/      wgpu, render engines, color management, render.render, frap.render_contact_sheet
│  ├─ frap-lang/        lexer, bpy-line parser, short-form parser, IR, value grammar
│  ├─ frap-proto/       JSON envelope, error taxonomy, describe/schema export
│  ├─ frap-cli/         `frap` binary (stdin loop, flags)
│  └─ frap-golden/      golden harness library + comparators (dev only)
└─ tests/golden/{cases,fixtures,props.toml}
```

**Dependency direction** (lower depends only on higher rows):

```
frap-cli
  └─ frap-lang, frap-proto
       └─ frap-wm ──────────────┬─ frap-ed-* (register ops into wm)
            ├─ frap-rna          ├─ frap-io-gltf ── frap-gltf
            ├─ frap-depsgraph    ├─ frap-render
            │    ├─ frap-modifiers, frap-anim
            │    └─ frap-bmesh, frap-uv
            ├─ frap-project
            ├─ frap-imbuf
            └─ frap-dna ── frap-geom ── frap-math ── frap-spec
```

Rules:
- `frap-spec` has **no hand-written logic**. Generated files sit under `src/generated/` and are committed, so that review shows spec drift. `build.rs` is not used, which keeps builds hermetic.
- `frap-ed-*` crates depend on `frap-wm`, and `frap-wm` knows nothing about them. Each exposes `pub fn register(r: &mut RegistryBuilder)`. `frap-cli` calls them all (the `wm_operatortypes_register` equivalent). Ops can therefore be developed in parallel crates.
- wgpu appears only in `frap-render` [RUST §3]. Draco, meshopt and basisu appear only in `frap-gltf` behind features.
- Default features build with only cargo plus a C/C++ compiler [RUST §12].

---

## 2. Core scene data model (DNA)

### 2.1 Main and ID storage

```rust
// crates/frap-dna/src/main.rs
pub struct Main {
    pub scenes: IdList<Scene>,       pub objects: IdList<Object>,
    pub meshes: IdList<Mesh>,        pub materials: IdList<Material>,
    pub node_groups: IdList<NodeTree>, pub images: IdList<Image>,
    pub textures: IdList<Texture>,   pub cameras: IdList<Camera>,
    pub lights: IdList<Light>,       pub worlds: IdList<World>,
    pub collections: IdList<Collection>, pub armatures: IdList<Armature>,
    pub actions: IdList<Action>,     pub shape_keys: IdList<Key>,
    pub filepath: Option<Utf8PathBuf>,
    pub is_dirty: bool,
    session_uid_counter: u32,            // deterministic, starts at 1 per session
}

/// Typed, generational handle. Survives renames and re-sorting. Never dangles silently.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct IdRef<T: IdType> { slot: u32, gen: u32, _t: PhantomData<fn() -> T> }

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum AnyId { Object(IdRef<Object>), Mesh(IdRef<Mesh>), Material(IdRef<Material>), /* … */ }

pub struct IdList<T: IdType> {
    slots: Vec<IdSlot<T>>,          // Arc<T> for copy-on-write snapshots (§15)
    order: Vec<u32>,                // Blender ListBase order = sorted by name (see 2.3)
}
struct IdSlot<T> { gen: u32, data: Option<Arc<T>> }

impl<T: IdType> IdList<T> {
    pub fn get(&self, r: IdRef<T>) -> Option<&T>;
    pub fn get_mut(&mut self, r: IdRef<T>) -> Option<&mut T>;   // Arc::make_mut + version bump
    pub fn find(&self, name: &str) -> Option<IdRef<T>>;          // bpy.data.X.find / ['name']
    pub fn iter_ordered(&self) -> impl Iterator<Item = (IdRef<T>, &T)>; // bpy.data.X[i] order
    pub fn version(&self, r: IdRef<T>) -> u64;                   // monotonic, for depsgraph caches
}

pub trait IdType: Clone + Send + Sync + 'static {
    const CODE: IdCode;              // OB, ME, MA, NT, IM, TE, CA, LA, WO, SCE, GR, AR, AC, KE
    const COLLECTION: &'static str;  // "objects"
    const DEFAULT_NAME: &'static str;// "Object", "Mesh", "Material" …
    fn header(&self) -> &IdHeader;
    fn header_mut(&mut self) -> &mut IdHeader;
    /// Enumerate outgoing ID references (BKE_library_foreach_ID_link). Drives user counts,
    /// remap, orphan purge and depsgraph relations.
    fn foreach_id(&self, f: &mut dyn FnMut(AnyId, IdUsage));
    fn anim_data(&self) -> Option<&AnimData> { None }
}

pub struct IdHeader {
    pub name: IdName,            // ≤255 bytes UTF-8, truncated on a char boundary [RNA §0.1]
    pub session_uid: u32,
    pub users: u32,              // maintained by Main::id_us_plus/min, never by hand
    pub flag: IdFlag,            // FAKEUSER, EXTRAUSER…
    pub properties: Option<IdProperty>, // custom props → glTF extras
}
```

User counting follows Blender's `id_us_plus`/`id_us_min`/`BKE_libblock_relink`. All mutations that change references go through `Main::assign_ref(owner, slot, new)` so counts cannot drift. A property test checks that `users == recount_from_foreach_id()` after random op sequences.

### 2.2 Naming

```rust
// crates/frap-dna/src/naming.rs
pub const MAX_ID_NAME: usize = 255;    // bytes
pub const MAX_SUB_NAME: usize = 63;    // bones, vgroups, shape keys, modifiers, nodes, UV layers, view layers

/// BLI_uniquename semantics [RNA §0.2], verified:
/// split at last ".NNN", take the LOWEST free number >= 1, zero pad to 3,
/// "Foo.001" requested while taken -> "Foo.003" if .001/.002 taken; "Bar.5" dup -> "Bar.001".
pub fn unique_name(requested: &str, max_bytes: usize, taken: impl Fn(&str) -> bool,
                   empty_default: &str) -> String;
pub fn truncate_utf8(s: &str, max_bytes: usize) -> &str;
```

`ID.name` assignment renames the assigned ID (it gets `.001`), and the other ID keeps its name. `ID.rename(name, mode)` implements `NEVER|ALWAYS|SAME_ROOT` and returns Blender's status enum.

### 2.3 Ordering of `bpy.data` collections (verified)

Blender keeps each ID ListBase sorted by name. The probe (Appendix A, Q3) gave `['a10','a9','b','B','Cube','Cube.002','Cube.010','Cube.9']`: a **case-insensitive, non-natural** comparison, with equal keys kept in insertion order. Renames re-sort (P10). `IdList::order` reproduces this. Two other orders exist and are kept separate: the **view-layer base order** (selection order, §5) and the glTF exporter's **natural case-insensitive root order** [GLTF §4.0].

### 2.4 Main structs (abridged; fields follow DNA names, RNA names map in `frap-rna`)

```rust
pub struct Object {
    pub id: IdHeader,
    pub data: ObData,                       // determines Object.type (RO in RNA)
    pub loc: [f32; 3], pub dloc: [f32; 3],
    pub rot: RotationStore,                 // mode + euler + quat + axis_angle, all kept (stale ones too)
    pub drot: RotationStore,
    pub scale: [f32; 3], pub dscale: [f32; 3],
    pub parent: Option<IdRef<Object>>, pub parent_type: ParentType,
    pub parent_bone: SubName, pub par_verts: [i32; 3],
    pub parent_inverse: Mat4,               // identity on RNA assign; inverse(parent world) via parent_set
    pub modifiers: Vec<ModifierData>,
    pub material_slots: Vec<ObjectMatSlot>, // link OBJECT/DATA flag + object-level material
    pub actcol: u16,                        // active_material_index
    pub mode: ObjectMode,                   // OBJECT | EDIT | POSE (RO in RNA; changed by mode_set)
    pub pose: Option<Pose>,
    pub shapenr: u16,                       // active_shape_key_index
    pub vertex_group_active: u16,           // names live on Mesh (verified, Appendix A R1)
    pub display: ObjectDisplay, pub empty: EmptyDisplay, pub color: [f32; 4],
    pub hide_viewport: bool, pub hide_render: bool, pub hide_select: bool,
    pub adt: Option<AnimData>,
    pub runtime: ObjectRuntime,             // object_to_world, bounds: copied back by depsgraph (§4)
}
pub enum ObData { Empty, Mesh(IdRef<Mesh>), Camera(IdRef<Camera>), Light(IdRef<Light>),
                  Armature(IdRef<Armature>) }

pub struct Scene {
    pub id: IdHeader,
    pub master_collection: Collection,      // embedded "Scene Collection"
    pub view_layers: Vec<ViewLayer>,
    pub camera: Option<IdRef<Object>>, pub world: Option<IdRef<World>>,
    pub cursor: View3DCursor, pub tool_settings: ToolSettings,
    pub r: RenderSettings, pub eevee: SceneEEVEE, pub display: SceneDisplay,
    pub view_settings: ColorManagedViewSettings, pub display_settings: ColorManagedDisplaySettings,
    pub unit: UnitSettings,
    pub frame: FrameSettings,               // start/end/current/step, fps/fps_base (r.*)
    pub adt: Option<AnimData>,
    pub custom_props: Option<IdProperty>,   // e.g. glTF variants
}
pub struct ViewLayer {
    pub name: SubName,
    pub bases: Vec<Base>,                   // ORDER = selection order (verified [OBJ §1.14])
    pub active: Option<IdRef<Object>>,      // may be unselected
    pub layer_collection: LayerCollection, pub active_layer_collection: LayerCollectionPath,
}
pub struct Base { pub object: IdRef<Object>, pub flag: BaseFlag /* SELECTED|HIDDEN(hide_set)|… */ }
```

Other IDs: `Mesh` (§3), `Material { node_tree: NodeTree (embedded), surface_render_method, blend_method (deprecated mirror), use_backface_culling, diffuse_color, … }`, `NodeTree { nodes: Vec<Node>, links: Vec<NodeLink>, interface }` (§7.6), `Image { source, packed: Option<PackedFile>, colorspace, alpha_mode, generated, filepath }`, `Camera`, `Light` (re-typed struct per `type`), `World`, `Collection`, `Armature { bones: BoneTree, edit_bones: Option<Vec<EditBone>>, collections }`, `Action` (§4.5), `Key { key_blocks: Vec<KeyBlock>, reference, use_relative }`, `Texture` (only for DISPLACE).

All "fresh" defaults come from `spec/blender-5.2/*.json` instance dumps, not RNA defaults, because they differ [RNA header]. Examples: `Modifier.show_viewport=True`, `Material.blend_method='HASHED'`, `light_add` 10 W vs factory Light 1000 W, `Light.shadow_soft_size=0.0`.

### 2.5 Factory startup

`frap-dna::startup::factory(main: &mut Main, wm: &mut WindowManager)` builds the scene from `spec/blender-5.2/factory_startup.json`. That covers Cube (factory vertex order, which differs from `primitive_cube_add` [MESH §1.4]), Camera, Light 1000 W, `Material`, `World`, `Collection`, `ViewLayer`, render settings, AgX, and the Layout viewport's `RegionView3D` matrices. `--empty` maps to `read_homefile(use_empty=True)`.

---

## 3. Mesh representation

There are three representations, as in Blender.

| Representation | Type | Used when | Owner |
|---|---|---|---|
| Original mesh (DNA) | `Mesh` (SoA attributes on 4 domains) | object mode, file I/O, RNA `mesh.vertices[...]` | `frap-dna` |
| Edit mesh | `BMEditMesh { bm: BMesh, … }` in `Mesh.runtime.edit_mesh` | Edit Mode ops | `frap-bmesh` |
| Evaluated mesh | `Arc<Mesh>` in the depsgraph | modifiers applied: render, export_apply, `evaluated_get`, `to_mesh` | `frap-depsgraph` |

### 3.1 `Mesh` (Blender 4.x/5.x attribute model)

```rust
// crates/frap-dna/src/mesh.rs
pub struct Mesh {
    pub id: IdHeader,
    pub verts_num: u32, pub edges_num: u32, pub faces_num: u32, pub corners_num: u32,
    pub face_offsets: Shared<[i32]>,         // len faces_num+1 (polygons[i].loop_start/total)
    pub vert_data: AttributeSet,             // "position" (FLOAT_VECTOR), crease_vert, …
    pub edge_data: AttributeSet,             // ".edge_verts" (INT32_2D), sharp_edge, uv_seam, crease_edge…
    pub face_data: AttributeSet,             // sharp_face, material_index, ".select_poly"…
    pub corner_data: AttributeSet,           // ".corner_vert", ".corner_edge", UV maps (FLOAT2), custom_normal
    pub deform_verts: Option<Shared<[DeformVert]>>, // MDeformVert, not an attribute
    pub vertex_group_names: Vec<SubName>,    // stored on the mesh (verified: linked dups share them)
    pub materials: Vec<Option<IdRef<Material>>>,
    pub key: Option<IdRef<Key>>,
    pub uv_active: Option<AttrName>, pub uv_active_render: Option<AttrName>,
    pub active_color: Option<AttrName>, pub default_color: Option<AttrName>,
    pub attributes_active_index: i32,
    pub runtime: MeshRuntime,
}
pub type Shared<T> = Arc<T>;                 // implicit sharing (Blender ImplicitSharingInfo)

pub struct Attribute { pub name: AttrName, pub domain: AttrDomain, pub data: Shared<AttrArray> }
pub enum AttrArray { Float(Vec<f32>), Int(Vec<i32>), Bool(Vec<bool>), FloatVector(Vec<[f32;3]>),
    FloatColor(Vec<[f32;4]>), ByteColor(Vec<[u8;4]>), Float2(Vec<[f32;2]>), Int16x2(Vec<[i16;2]>),
    Int32x2(Vec<[i32;2]>), Quaternion(Vec<[f32;4]>), Float4x4(Vec<[[f32;4];4]>), Int8(Vec<i8>),
    Float4(Vec<[f32;4]>), String(Vec<String>) }

pub struct MeshRuntime {
    pub edit_mesh: Option<Arc<BMEditMesh>>,
    pub normals: OnceCache<MeshNormals>,     // face/vert/corner, invalidated by tags
    pub looptris: OnceCache<Vec<[u32; 3]>>,
    pub bounds: OnceCache<Aabb>,
}
```

Rules copied from Blender [MESH §1.2–1.3, §2.5]:
- `sharp_face`, `sharp_edge` and `uv_seam` are **removed when every value is false**. `use_smooth`, `use_seam` and `use_edge_sharp` are RNA proxies that create or remove these attributes.
- `normals_domain` = FACE if every face is sharp, POINT if nothing is sharp and there are no custom normals, CORNER otherwise.
- Corner normals: an angle-weighted fan, split at sharp edges, sharp-face boundaries and non-manifold edges. This is a port of `mesh_normals.cc` into `frap-geom::normals`.
- **Looptris** (used by glTF export and the renderer): quads → `(0,1,2),(0,2,3)` (verified on two non-planar quads, Appendix A P7/P8). N-gons → `BLI_polyfill_2d` with **no** beautify. BMesh triangulate ops use polyfill **plus** beautify.

### 3.2 BMesh

```rust
// crates/frap-bmesh/src/mempool.rs
/// BLI_mempool emulation: iteration = slot order, alloc pops the most recently freed slot (LIFO),
/// otherwise appends. Verified ordering consequences in [MESH §2.4].
pub struct MemPool<T> { slots: Vec<Slot<T>>, free: Vec<u32>, live: u32 }
enum Slot<T> { Live(T), Free }
impl<T> MemPool<T> {
    pub fn alloc(&mut self, v: T) -> u32;     // free.pop() or push
    pub fn free(&mut self, i: u32);           // free.push(i)
    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)>;
}

// crates/frap-bmesh/src/types.rs
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)] pub struct VertId(u32);
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)] pub struct EdgeId(u32);
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)] pub struct LoopId(u32);
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)] pub struct FaceId(u32);

pub struct BMHeader { pub hflag: HFlag /* SELECT|HIDDEN|SEAM|SMOOTH|TAG|INTERNAL_TAG */, pub index: i32 }
pub struct BMVert { pub head: BMHeader, pub co: [f32; 3], pub no: [f32; 3], pub e: Option<EdgeId> }
pub struct BMEdge { pub head: BMHeader, pub v1: VertId, pub v2: VertId, pub l: Option<LoopId>,
                    pub v1_disk: DiskLink, pub v2_disk: DiskLink }
pub struct DiskLink { pub next: EdgeId, pub prev: EdgeId }
pub struct BMLoop { pub head: BMHeader, pub v: VertId, pub e: EdgeId, pub f: FaceId,
                    pub next: LoopId, pub prev: LoopId, pub radial_next: LoopId, pub radial_prev: LoopId }
pub struct BMFace { pub head: BMHeader, pub l_first: LoopId, pub len: u32, pub no: [f32; 3], pub mat_nr: i16 }

pub struct BMesh {
    pub verts: MemPool<BMVert>, pub edges: MemPool<BMEdge>,
    pub loops: MemPool<BMLoop>, pub faces: MemPool<BMFace>,
    pub vdata: CustomData, pub edata: CustomData, pub ldata: CustomData, pub pdata: CustomData,
    pub select_mode: SelectMode,                 // VERT|EDGE|FACE bitset
    pub select_history: Vec<BMEditSelection>,    // last = active element
    pub act_face: Option<FaceId>,
    pub shapenr: i32,                            // active shape key while editing
    elem_index_dirty: ElemTypes, elem_table_dirty: ElemTypes,
}
/// One column per layer, indexed by pool slot (BMesh CustomData blocks).
pub struct CustomData { pub layers: Vec<CdLayer> }
pub struct CdLayer { pub name: AttrName, pub ty: CdType, pub active: bool, pub active_rnd: bool,
                     pub data: CdColumn }
```

The **kernel API keeps Blender's names** so `bmesh_core.cc` and `bmesh_mods.cc` port line by line: `bm_vert_create`, `bm_edge_create`, `bm_face_create`, `bm_face_create_verts`, `bm_vert_kill`, `bm_edge_kill`, `bm_face_kill`, `bmesh_kernel_split_edge_make_vert` (SEMV), `bmesh_kernel_join_edge_kill_vert` (JEKV), `bmesh_kernel_split_face_make_edge` (SFME), `bmesh_kernel_join_face_kill_edge` (JFKE), `bm_vert_splice`, `bm_edge_splice`, `bm_face_normal_flip`, `bm_mesh_elem_index_ensure`.

**bmesh operators** are typed functions named like `bmesh.ops.*`, with slot names as fields. The golden harness can then call the same `bmesh.ops` in Blender as an emulation (loopcut, [MESH §0.8]):

```rust
// crates/frap-bmesh/src/ops/extrude.rs
pub struct ExtrudeFaceRegion<'a> {
    pub geom: &'a [BMElem], pub edges_exclude: &'a [EdgeId], pub use_keep_orig: bool,
    pub use_normal_flip: bool, pub use_normal_from_adjacent: bool,
    pub use_dissolve_ortho_edges: bool, pub use_select_history: bool, pub skip_input_flip: bool,
}
pub struct ExtrudeFaceRegionOut { pub geom: Vec<BMElem> }
pub fn extrude_face_region(bm: &mut BMesh, p: ExtrudeFaceRegion) -> Result<ExtrudeFaceRegionOut, BmError>;
```

Each op file starts with a **create/kill order note** taken from Blender's source and from the [MESH App. B] observations. One example: "region extrude: new verts in original vert iteration order; cap edges first then vertical edges; first side face reuses the freed slot of the killed face". A per-op golden test proves it.

### 3.3 Conversion and edit-mode semantics

- `bm_from_me(&Mesh, BmFromMeParams) -> BMesh` and `bm_to_me(&BMesh, &mut Mesh, BmToMeParams)` are ports of `BM_mesh_bm_from_me` / `BM_mesh_bm_to_me`. Elements are created in index order, selection comes from `.select_*`, hide from `.hide_*`, every attribute becomes a CustomData layer, and shape keys become `shape` layers. On write-back, elements are packed in pool iteration order, all-false boolean attributes are dropped, and basis edits propagate to the other shape keys.
- `object.mode_set(mode='EDIT')` creates `BMEditMesh` for **every selected object of the active object's type** (multi-object edit, `objects_in_mode`). Leaving edit mode writes back and drops it.
- D3: RNA reads of `Mesh` element data while `edit_mesh.is_some()` return the **original (stale) arrays**, exactly as Blender. `obj.update_from_editmode()` runs `bm_to_me` without leaving edit mode.
- Selection flush (`select_flush_mode`), `select_mode` conversion (vert→face keeps only fully selected faces), history remapping and the active face follow [MESH §2.2, §3.3]. They live in `frap-bmesh/src/select.rs`.

### 3.4 Evaluated mesh

The depsgraph produces `Arc<Mesh>` per object. With no modifiers or shape keys, it is the **same Arc** as the original (implicit sharing, zero copy). Deform-only stacks share topology arrays and replace only `position`. In edit mode, the evaluated input is `bm_to_me` of the edit mesh (Blender's mesh wrapper), followed by modifiers with `show_in_editmode`.

---

## 4. Depsgraph and modifier evaluation pipeline

### 4.1 Evaluation timing (verified, Appendix A P1–P6, Q1)

| Event | Blender 5.2 behaviour (verified) | frap |
|---|---|---|
| `o.location = …` | Original changes. `matrix_world` and `dimensions` stay stale. | Tag `TRANSFORM` only. |
| any `bpy.ops.*` call | The depsgraph is evaluated **before exec** (parent_set saw the fresh parent matrix) and is fresh after. | `ensure_evaluated()` before poll and after exec. |
| `view_layer.update()`, `context.evaluated_depsgraph_get()` | evaluates | evaluates |
| `scene.frame_set(f)` | Sets the frame, evaluates animation and **flushes animated values to the original**. | Same. |
| `render.render`, `export_scene.gltf` | Evaluate (export calls `frame_set(0)` [GLTF §4.0]). | Same. |

### 4.2 Depsgraph API

```rust
// crates/frap-depsgraph/src/lib.rs
bitflags! { pub struct Recalc: u32 { const TRANSFORM=1; const GEOMETRY=2; const SHADING=4;
    const ANIMATION=8; const SELECT=16; const BASE_FLAGS=32; const FRAME_CHANGE=64; const PARAMETERS=128; } }

pub struct Depsgraph {
    frame: f32, ctime: f32,
    tags: IndexMap<AnyId, Recalc>,
    relations: Relations,                         // rebuilt when topology tagged (parenting, modifier refs)
    eval: IndexMap<AnyId, EvalEntry>,            // keyed by ID, validated by IdList::version
}
pub struct EvalObject { pub object_to_world: Mat4, pub mesh: Option<Arc<Mesh>>, pub bounds: Option<Aabb>,
                        pub modifier_status: Vec<(SubName, ModifierStatus)> }

impl Depsgraph {
    pub fn tag(&mut self, id: AnyId, r: Recalc);
    pub fn tag_relations(&mut self);
    /// Evaluate everything tagged, in relation order; copy results back to originals
    /// (BKE_object_sync_to_original: object_to_world, bounds, dimensions source).
    pub fn ensure_evaluated(&mut self, main: &mut Main, scene: IdRef<Scene>) -> EvalReport;
    pub fn frame_set(&mut self, main: &mut Main, scene: IdRef<Scene>, frame: i32, subframe: f32);
    pub fn object(&self, ob: IdRef<Object>) -> Option<&EvalObject>;
}
```

Relations: parent → child; a modifier object reference (boolean operand, mirror object, array offset object, armature, displace texture coords object) → owner; shape key Key → mesh object; action → animated ID. Order is a deterministic topological sort (ties broken by `bpy.data` order). A cycle gives Blender's `Dependency cycle detected` warning in `reports`, and the cycle is evaluated in data order.

Per-object pipeline, in Blender's order:
1. **Animation**: evaluate AnimData (action slot channelbag F-curves, then NLA) when `ANIMATION|FRAME_CHANGE` is tagged. Values are written through RNA paths (§7) into the original (active-depsgraph flush), then copied.
2. **Transform**: `matrix_basis` from loc/rot/scale/deltas, then the parent chain with `parent_inverse`, writing `object_to_world`. Parenting to a bone or vertex uses the evaluated pose or mesh.
3. **Geometry** (mesh objects): original mesh (or edit-mesh wrapper), shape-key mix (`BKE_key` relative evaluation), the modifier stack, then normals and bounds caches.
4. Copy-back to the original runtime for RNA reads.

### 4.3 Modifier stack

```rust
// crates/frap-modifiers/src/lib.rs
pub enum ModifierKind { OnlyDeform, Constructive, NonGeometrical }
bitflags! { pub struct ModifierTypeFlags: u32 { const ACCEPTS_MESH=1; const SUPPORTS_EDITMODE=2;
    const ENABLE_IN_EDITMODE=4; const SUPPORTS_MAPPING=8; const ACCEPTS_CVS=16; } }

pub struct ModifierTypeInfo {
    pub ty: ModifierType,                 // SUBSURF, MIRROR, … (RNA enum identifier)
    pub struct_name: &'static str,        // "SubsurfModifier"
    pub default_name: &'static str,       // "Subdivision", "SimpleDeform", "EdgeSplit" [RNA §5.1]
    pub kind: ModifierKind, pub flags: ModifierTypeFlags,
    pub init_data: fn(&mut ModifierSettings),                    // fresh defaults from spec
    pub required_data_mask: fn(&ModifierData) -> DataMask,
    pub is_disabled: fn(&ModifierData, &Main) -> Option<&'static str>, // e.g. "Boolean: no operand"
    pub foreach_id: fn(&ModifierData, &mut dyn FnMut(AnyId)),
    pub deform_verts: Option<fn(&ModifierData, &ModEvalCtx, &Mesh, &mut [[f32; 3]])>,
    pub modify_mesh: Option<fn(&ModifierData, &ModEvalCtx, Arc<Mesh>) -> Result<Arc<Mesh>, ModError>>,
}
pub struct ModifierData {
    pub name: SubName, pub persistent_uid: u32,
    pub show_viewport: bool, pub show_render: bool, pub show_in_editmode: bool, pub show_on_cage: bool,
    pub show_expanded: bool, pub is_active: bool, pub use_pin_to_last: bool,
    pub settings: ModifierSettings,       // enum with one variant per type (generated field lists)
}
pub struct ModEvalCtx<'a> { pub main: &'a Main, pub deg: &'a Depsgraph, pub object: IdRef<Object>,
                            pub use_render: bool, pub in_editmode: bool, pub frame: f32 }
```

Evaluation follows `BKE_mesh_wrapper` / `mesh_calc_modifiers`: leading deform-only modifiers run on positions, then each constructive modifier gets the mesh. Disabled modifiers (`show_viewport` false, or `is_disabled` reporting a reason) are skipped, and the reason goes into `EvalObject.modifier_status` and into the JSON response. `use_pin_to_last` keeps "Smooth by Angle" last.

**Implementation level per modifier** (principle 3):

| Modifier | Blender level | frap module | Library/port |
|---|---|---|---|
| ARRAY, MIRROR (bisect via BMesh), SOLIDIFY (simple & non-manifold), WELD, EDGE_SPLIT, DISPLACE, WEIGHTED_NORMAL, ARMATURE, SMOOTH, CAST, SIMPLE_DEFORM | Mesh | `mod_array.rs` … | port |
| BEVEL, TRIANGULATE, DECIMATE | BMesh | uses `frap-bmesh` ops (shared with `mesh.bevel`, `quads_convert_to_tris`, `mesh.decimate`) | port |
| SUBSURF | OpenSubdiv + `subdiv_mesh` layout | `mod_subsurf.rs` | `opensubdiv-rs` spike + own ordering pass, or own Catmull-Clark [RUST §6.4] |
| BOOLEAN | EXACT: mesh_boolean; FLOAT: BMesh intersect; MANIFOLD: manifold | `mod_boolean/` | `manifold-rust` for MANIFOLD (and EXACT initially, tier T3); FLOAT later |
| NODES ("Smooth by Angle" only) | Geometry Nodes | `mod_nodes.rs` with built-in group registry | own |

`NODES` modifier: `bpy.data.node_groups["Smooth by Angle"]` exists as a NodeTree ID with the essentials interface (`Input_0`, `Input_1` Angle 0.5236, `Socket_1` Ignore Sharpness, `Output_2`). `mod.properties.inputs.Input_1.value` is the RNA path; `mod["Input_1"]` gives Blender's `TypeError: this type doesn't support IDProperties` [RNA §0.6]. The evaluator dispatches on the node-group **name**, and unknown groups give a modifier error, `Unsupported node group "X" (frap supports: Smooth by Angle)`.

### 4.4 Animation data (frap-anim, data in frap-dna)

The slotted model follows [RNA §13]:

```rust
pub struct Action { pub id: IdHeader, pub slots: Vec<ActionSlot>, pub layers: Vec<ActionLayer>,
                    pub frame_range: Option<(f32, f32)>, pub use_cyclic: bool }
pub struct ActionSlot { pub handle: u32, pub identifier: String /* "OBCube" */, pub target_id_type: IdCode }
pub struct ActionLayer { pub name: SubName, pub strips: Vec<ActionStrip> }
pub struct ActionStrip { pub kind: StripKind /* KEYFRAME */, pub channelbags: Vec<Channelbag> }
pub struct Channelbag { pub slot_handle: u32, pub groups: Vec<ActionGroup>, pub fcurves: Vec<FCurve> }
pub struct FCurve { pub data_path: String, pub array_index: i32, pub group: Option<u32>,
                    pub keyframe_points: Vec<BezTriple>, pub extrapolation: Extrapolation, pub mute: bool, … }
pub struct AnimData { pub action: Option<IdRef<Action>>, pub slot_handle: Option<u32>,
                      pub last_slot_identifier: String, pub nla_tracks: Vec<NlaTrack>, pub action_influence: f32, … }
```

`keyframe_insert(data_path, *, index=-1, frame=None, group="", options=set(), keytype='KEYFRAME')` creates `<IDName>Action`, the slot `OB<name>` / `KE<name>` / `NT<name>`, one layer "Layer", one KEYFRAME strip, the group "Object Transforms" (or the bone name), and BEZIER/AUTO_CLAMPED keys. Shape keys go into the **object's** action under a `KE` slot. Material node trees get their own action. `frap-anim::handles` is a port of `BKE_nurb_handle_calc` (AUTO_CLAMPED), because sampled glTF output depends on it. Accessing `action.fcurves` gives `AttributeError: 'Action' object has no attribute 'fcurves'` with a frap hint naming `action.layers[0].strips[0].channelbag(slot).fcurves`.

---

## 5. Context, selection and mode model

```rust
// crates/frap-wm/src/wm.rs
pub struct WindowManager {
    pub scene: IdRef<Scene>, pub view_layer: usize,
    pub view3d: VirtualView3D,          // factory Layout viewport, so "V" ops pass poll [OBJ §1.7]
    pub preferences: Preferences,       // edit.use_duplicate_*, keyframe_new_interpolation_type…
    pub render_result: Option<IdRef<Image>>, // "Render Result"
}
pub struct VirtualView3D { pub view_matrix: Mat4, pub view_rotation: Quat, pub view_location: Vec3,
    pub view_distance: f32, pub is_perspective: bool, pub lens: f32, pub clip: (f32, f32),
    pub shading: View3DShading }        // space_data.shading, reused by Workbench defaults

// crates/frap-wm/src/context.rs
pub struct Context<'a> { pub main: &'a Main, pub wm: &'a WindowManager, pub deg: &'a Depsgraph }
impl<'a> Context<'a> {
    pub fn scene(&self) -> IdRef<Scene>;
    pub fn view_layer(&self) -> &ViewLayer;
    pub fn active_object(&self) -> Option<IdRef<Object>>;            // = object
    pub fn selected_objects(&self) -> Vec<IdRef<Object>>;            // base order
    pub fn selected_editable_objects(&self) -> Vec<IdRef<Object>>;
    pub fn visible_objects(&self) -> Vec<IdRef<Object>>;
    pub fn edit_object(&self) -> Option<IdRef<Object>>;
    pub fn objects_in_mode(&self) -> Vec<IdRef<Object>>;
    pub fn objects_in_mode_unique_data(&self) -> Vec<IdRef<Object>>;
    pub fn mode(&self) -> CtxMode;   // OBJECT | EDIT_MESH | EDIT_ARMATURE | POSE
    pub fn collection(&self) -> CollectionRef;   // active layer collection: where *_add links objects
    pub fn tool_settings(&self) -> &ToolSettings;
    pub fn active_bone(&self) -> Option<BoneRef>; pub fn active_pose_bone(&self) -> Option<BoneRef>;
}
```

- Selection and hiding are **per view layer** (`Base.flag`). `Object.select_set/select_get/hide_set/hide_get/visible_get` are RNA functions working on the base of the current view layer. `selected_objects` is in base order, not click order (verified [OBJ §1.14]).
- Base order = the order objects were synced into the view layer (layer-collection traversal, then link order). `frap-dna::view_layer::sync(…)` ports `BKE_layer_collection_sync`.
- `context` is read-only, as in Blender. Writes go through `view_layer.objects.active = …` and `select_set`. Assigning `bpy.context.object = …` gives `AttributeError: bpy_struct: Context property "object" is read-only` (verified, S3).
- UI-only members (`area`, `region`, `space_data`) are `None`. `context.space_data.shading` is the one exception: it returns the virtual view's shading, because Workbench defaults read it.
- Edit-mode element selection lives in each BMesh. `tool_settings.mesh_select_mode` is the scene default, applied on mode enter and by `mesh.select_mode`.
- The 3D cursor (`scene.cursor`) feeds primitive `location` when it is not set, `origin_set(ORIGIN_CURSOR)`, pivot `CURSOR` and merge `CURSOR`.

---

## 6. Operator registry (wm)

### 6.1 Definitions (generated) and implementations (hand-written)

```rust
// crates/frap-spec/src/generated/ops.rs (generated from spec/blender-5.2/ops.json + overlay)
pub struct OpDef {
    pub idname: &'static str,            // "mesh.primitive_cube_add"
    pub c_idname: &'static str,          // "MESH_OT_primitive_cube_add"
    pub label: &'static str, pub description: &'static str,
    pub options: OpOptions,              // REGISTER|UNDO|INTERNAL|MACRO…
    pub props: &'static [PropDef],       // shared PropDef type with RNA (§7)
    pub macro_steps: &'static [&'static str], // ["MESH_OT_extrude_region","TRANSFORM_OT_translate"]
    pub poll_matrix: PollMatrix,         // dumped A–E results, for tests and --describe
    pub overlay: OpOverlay,              // status, tier, notes, frap_ props, headless semantics
}

// Typed props struct per op, generated:
pub struct PrimitiveCubeAdd {
    pub size: Prop<f32>, pub calc_uvs: Prop<bool>, pub enter_editmode: Prop<bool>,
    pub align: Prop<AddAlign>, pub location: Prop<[f32; 3]>, pub rotation: Prop<[f32; 3]>,
    pub scale: Prop<[f32; 3]>,
}
#[derive(Clone, Debug)]
pub struct Prop<T> { pub value: T, pub is_set: bool }    // RNA_struct_property_is_set [OBJ §1.3]
```

```rust
// crates/frap-wm/src/operator.rs
pub enum OpStatus { Finished, Cancelled }
pub enum OpError {
    /// RuntimeError("Error: <msg>") — exec failure with ERROR report
    Report(String),
    /// TypeError / ValueError during property conversion (raised before poll)
    Arg(ArgError),
    /// frap: operator registered from spec but not implemented
    NotImplemented { idname: &'static str },
}
pub type OpResult = Result<OpStatus, OpError>;

pub trait Operator: 'static {
    type Props: OpProps;                         // generated struct
    const IDNAME: &'static str;
    fn poll(_ctx: &Context) -> Result<(), PollFail> { Ok(()) }
    /// Dynamic enum items (mode_set.mode, modifier_add.type, merge.type) [OBJ §1.4]
    fn enum_items(_ctx: &Context, _prop: &str) -> Option<Vec<EnumItem>> { None }
    fn exec(ctx: &mut ExecCtx, props: &Self::Props) -> OpResult;
    /// Op-specific JSON summary (created IDs, counts) appended to the response.
    fn summarize(_ctx: &Context, _props: &Self::Props, _out: &mut OpSummary) {}
}
pub struct PollFail { pub message: Option<String> }   // None → "failed, context is incorrect"

pub struct ExecCtx<'a> {
    pub main: &'a mut Main, pub wm: &'a mut WindowManager, pub deg: &'a mut Depsgraph,
    pub reports: &'a mut ReportList,              // INFO/WARNING/ERROR like BKE_report
}
impl ExecCtx<'_> { pub fn ctx(&self) -> Context<'_>; pub fn tag(&mut self, id: AnyId, r: Recalc); }

pub trait OpProps: Sized {
    fn def() -> &'static OpDef;
    /// Python-kwargs → props with Blender's conversion rules and messages.
    fn from_args(args: &Args, ctx: &Context, items: &dyn DynEnumSource) -> Result<Self, ArgError>;
    fn to_values(&self) -> IndexMap<&'static str, Value>;   // for responses / redo record
}
```

Example implementation:

```rust
// crates/frap-ed-mesh/src/primitives.rs
pub struct PrimitiveCubeAddOp;
impl Operator for PrimitiveCubeAddOp {
    type Props = props::mesh::PrimitiveCubeAdd;
    const IDNAME: &'static str = "mesh.primitive_cube_add";
    fn exec(ctx: &mut ExecCtx, p: &Self::Props) -> OpResult {
        let xf = object_add_generic_get_opts(ctx, &p.align, &p.location, &p.rotation, &p.scale)?; // cursor/align rules
        let (ob, bm) = make_prim_target(ctx, "Cube", p.enter_editmode.value)?;  // object mode: new ob; edit: merge
        frap_bmesh::ops::primitive::create_cube(bm, CreateCube { size: p.size.value * xf.scale_fac,
            matrix: xf.local_matrix, calc_uvs: p.calc_uvs.value })?;
        Ok(OpStatus::Finished)
    }
}
```

### 6.2 Registry and invocation pipeline

```rust
pub struct Registry { ops: IndexMap<&'static str, OpEntry> }   // keyed by python idname
pub struct OpEntry { pub def: &'static OpDef, pub kind: OpImpl }
pub enum OpImpl { Native(&'static dyn ErasedOperator), Macro(&'static [&'static str]), Unimplemented }

impl Session {
    /// Shared by every front-end (bpy-line, short form, future Lua).
    pub fn call_operator(&mut self, idname: &str, args: Args) -> OpOutcome;
}
```

Pipeline (each step maps to a Blender step, so error classes match):
1. **Lookup.** Unknown → `AttributeError: Calling operator "bpy.ops.mesh.foo" error, could not be found`. Known but `Unimplemented` → `NotImplementedError` (code `OP_NOT_IMPLEMENTED`).
2. **Positional args** → `TypeError` (Blender ops are keyword-only).
3. **Depsgraph ensure** (verified: Blender evaluates before exec).
4. **Convert args** with `OpProps::from_args`. Unknown keyword → `TypeError: Converting py args to operator properties:: keyword "sizee" unrecognized` (double colon verified). Wrong type → `TypeError: MESH_OT_primitive_cube_add.size expected a float type, not str`. Wrong enum → `TypeError: … enum "EDIT" not found in ('OBJECT')`. Array length → `ValueError`. Out-of-range float → **clamped** (RNA hard range). Macro sub-dicts (`TRANSFORM_OT_translate={...}`) recurse.
5. **Poll** → `RuntimeError: Operator bpy.ops.X.poll() failed, context is incorrect` or a custom message (`Object has no vertex groups`).
6. **Begin transaction** (§15).
7. **Exec.** Macros run their steps in order, each with its nested props. A step that cancels cancels the macro.
8. **Result**: `FINISHED` commits. `CANCELLED` rolls back with Blender's WARNING reports, or a synthesized one where Blender is silent [OBJ §1.5]. `Err(Report)` → `RuntimeError: Error: <msg>`, rolled back.
9. **Depsgraph ensure**, then build the response (§9).

**Dynamic enums.** `ops.json` stores the static superset. `Operator::enum_items` filters by context (mode_set for Camera → only `OBJECT`). `--describe` publishes the superset plus a `context_rule` string from the overlay.

**Headless deviations** (`mesh.loopcut` and `loopcut_slide` with `edge_index`, `spin`/`screw` requiring `axis`, the pick ops by index, `uv.project_from_view` from `scene.camera` with `camera_bounds=True`) are implemented from their hidden index props. The overlay marks them `headless: "frap-extended"` with a `frap_note`, and the golden harness uses Blender-side emulations (§14.4).

---

## 7. RNA: typed property paths

### 7.1 Definitions (generated)

```rust
// crates/frap-spec/src/generated/rna.rs
pub struct StructDef {
    pub identifier: &'static str,          // "Object", "BevelModifier", "ShaderNodeBsdfPrincipled"
    pub base: Option<StructId>,            // "Modifier", "ShaderNode" → "Node"
    pub name_property: Option<PropIdx>,    // used for coll['Name'] lookup and repr
    pub props: &'static [PropDef], pub funcs: &'static [FuncDef],
    pub refine: Option<fn(&RnaRead, &RnaPtr) -> StructId>,  // Modifier → BevelModifier, Light → SpotLight
}
pub struct PropDef {
    pub identifier: &'static str, pub name: &'static str, pub description: &'static str,
    pub kind: PropKind, pub subtype: Subtype, pub unit: Unit,
    pub array: Option<ArrayDims>,          // [3], [4], [4,4], dynamic
    pub flags: PropFlags,                  // READONLY|HIDDEN|SKIP_SAVE|ENUM_FLAG|ANIMATABLE|PYTHON_ONLY|NEVER_NULL
    pub default: DefaultValue,             // fresh-instance default (spec), RNA default kept separately
}
pub enum PropKind { Bool, Int { hard: (i64, i64), soft: (i64, i64) },
    Float { hard: (f64, f64), soft: (f64, f64), precision: u8, f64_storage: bool },
    String { max_bytes: u32 }, Enum { items: &'static [EnumItem], dynamic: Option<DynEnumId> },
    Pointer { ty: StructId, id_ref: bool }, Collection { ty: StructId, api: Option<StructId> } }
pub struct FuncDef { pub identifier: &'static str, pub params: &'static [ParamDef],
    pub ret: Option<PropDef>, pub keyword_only_from: Option<u8>, pub positional_only_upto: Option<u8>,
    pub uses_context: bool, pub python_only: bool }
```

`python_only.toml` adds non-RNA members agents use: `Object.children`, `children_recursive`, `users_collection`, `users_scene`, `Mesh.from_pydata/shade_smooth/shade_flat/edge_keys`, and `bpy_struct.keyframe_insert/keyframe_delete` with the 5.2 keyword-only signature [RNA §0.5, §0.11].

### 7.2 Runtime pointers and resolution

Blender's `PointerRNA` is `(owner_id, type, data*)`. Raw pointers do not fit Rust borrowing, so frap stores **owned addresses** that are re-resolved on every access. That is not a hot path.

```rust
// crates/frap-rna/src/ptr.rs
#[derive(Clone, Debug, PartialEq)]
pub struct RnaPtr { pub owner: Owner, pub ty: StructId, pub loc: SmallVec<[Seg; 4]> }
pub enum Owner { Id(AnyId), Context, BlendData, WindowManager }
pub enum Seg {                          // stable keys, never raw indices into mutable Vecs
    Modifier(u32 /* persistent_uid */), Node(u32 /* node uid */), Socket { output: bool, index: u16 },
    Link(u32), Vertex(u32), Edge(u32), Polygon(u32), Loop(u32), Attribute(AttrName), AttrElem(u32),
    UvLayer(AttrName), MaterialSlot(u16), VertexGroup(u16), KeyBlock(u16), Bone(BoneName),
    PoseBone(BoneName), EditBone(BoneName), Slot(u32), Layer(u16), Strip(u16), Channelbag(u32),
    FCurve(u16), Keyframe(u32), NlaTrack(u16), NlaStrip(u16, u16), Field(PropIdx) /* nested struct */,
}

pub struct RnaRead<'a>  { pub main: &'a Main, pub wm: &'a WindowManager, pub deg: &'a Depsgraph }
pub struct RnaWrite<'a> { pub main: &'a mut Main, pub wm: &'a mut WindowManager,
                          pub deg: &'a mut Depsgraph, pub reports: &'a mut ReportList }

pub enum Resolved { Struct(RnaPtr), Prop { ptr: RnaPtr, prop: PropIdx, index: Option<u32> },
                    Coll { ptr: RnaPtr, prop: PropIdx }, Func { ptr: RnaPtr, func: FuncIdx } }

pub fn resolve(cx: &RnaRead, path: &RnaPath) -> Result<Resolved, RnaError>;
pub fn get(cx: &RnaRead, r: &Resolved) -> Result<Value, RnaError>;
pub fn set(cx: &mut RnaWrite, r: &Resolved, v: Value) -> Result<(), RnaError>;
pub fn call(cx: &mut RnaWrite, ptr: &RnaPtr, f: FuncIdx, args: Args) -> Result<Value, RnaError>;
```

### 7.3 Struct implementations: `rna_*.rs` like Blender's `makesrna`

Each struct has a hand-written file mirroring `rna_object.cc` and friends. A small builder links closures to **generated** prop indices. A test checks that every PropDef marked `implemented` in the overlay has getters and setters, and that no closure refers to an unknown name.

```rust
// crates/frap-rna/src/structs/rna_object.rs
pub fn define(s: &mut StructImpl<Object>) {
    s.prop("location").get(|ob, _| Value::vec_f32(&ob.loc, Subtype::Translation))
        .set(|ob, v, cx| { ob.loc = v.as_f32_array::<3>()?; cx.tag(Recalc::TRANSFORM); Ok(()) });
    s.prop("rotation_mode").set(|ob, v, cx| { ob.rot.set_mode_converting(v.as_enum()?)?; cx.tag(Recalc::TRANSFORM); Ok(()) });
    s.prop("parent").set(|ob, v, cx| { cx.assign_parent_keep_inverse_identity(ob, v.as_id()?) }); // RNA: no inverse
    s.prop("matrix_world").get(|ob, _| Value::matrix(ob.runtime.object_to_world))       // stale until eval (D4)
        .set(|ob, v, cx| ob_apply_world_matrix(ob, v.as_mat4()?, cx));
    s.prop("dimensions").get(|ob, cx| dims_from_bounds(ob, cx)).set(set_dimensions);
    s.func("select_set").call(|cx, ob, a| base_select_set(cx, ob, a.bool("state")?, a.opt_view_layer()?));
    s.func("shape_key_add").call(shape_key_add);
    s.coll("modifiers").lookup_by_name().api_struct("ObjectModifiers");
}
```

Assignment rules (verified [RNA §1.2]): any sequence of the right length is accepted and ints coerce to float; wrong length → `ValueError: … sequences of dimension 0 should contain 3 items, not 2`; scalar to array → `TypeError`; read-only → `AttributeError: bpy_struct: attribute "type" from "Object" is read-only`; bad enum → `TypeError: … enum "FOO" not found in (…)` with the item list; float hard range → clamp; `rotation_mode` change converts the rotation. Floats are stored as **f32** unless `f64_storage`.

Setters return **update tags** that mirror RNA `update` callbacks (`rna_Object_internal_update` → TRANSFORM, mesh data → GEOMETRY, material → SHADING, base select → SELECT).

### 7.4 Path grammar (shared by both front-ends)

```
path     := root accessor*
root     := 'bpy.data' | 'bpy.context' | 'D' | 'C'              (short form: 'data' | 'context')
accessor := '.' IDENT                                         # property or function or python-only member
          | '[' STRING ']'                                    # collection lookup by name (name_property)
          | '[' INT ']'                                       # index (negative allowed) / array element
          | '[' INT ':' INT ']'                               # array slice (location[0:2])
call     := path '(' args ')'                                 # RNA function / collection API function
```

Lookup semantics:
- `coll['Name']` → `KeyError: 'bpy_prop_collection[key]: key "Nope" not found'`. `coll.get('Name', default)` returns None. `coll.find('Name')` returns -1 when missing. `coll[i]` works in `bpy.data` name order.
- Node sockets: `inputs['Base Color']` returns the **first enabled** socket with that name. Identifiers are not keys, and integer indices work [RNA §0.10]. Socket enablement rules (Mix `data_type`, Principled `subsurface_method`…) come from `nodes.json` plus a hand-written `availability.rs` per node type.
- GN modifier inputs: `mod.properties.inputs.Input_1.value` and `inputs['Input_1']`. Display names are not keys.
- `repr` strings for refs follow Blender: single quotes at the root (`bpy.data.objects['Cube']`), double quotes below (`.modifiers["Bevel"]`). `path_from_id` builds F-curve paths **byte-identically** (`nodes["Principled BSDF"].inputs[0].default_value`), because the glTF exporter pattern-matches on them.

### 7.5 Values

```rust
// crates/frap-rna/src/value.rs
pub enum Value {
    None, Bool(bool), Int(i64), F32(f32), F64(f64), Str(String),
    Enum(String), EnumSet(BTreeSet<String>),
    Array { data: ArrayData, dims: ArrayDims, py: PyArrayKind /* Vector|Euler|Quaternion|Color|Matrix|PropArray */ },
    Euler { v: [f32; 3], order: EulerOrder },
    Ptr(RnaPtr), Coll { owner: RnaPtr, prop: PropIdx },
    Tuple(Vec<Value>), List(Vec<Value>), Dict(IndexMap<String, Value>), // literals & args
}
```

`frap-math` reproduces the **mathutils** constructors and operators used in scripts: `Vector((…))`, `Euler((…), 'XYZ')`, `Quaternion((w,x,y,z))`, `Matrix(…)`, `Color((…))`, `+ - * / @`, unary `-`, `.normalized()`, `.to_quaternion()`, `.to_euler()`, `.to_matrix()`, `Matrix.Translation/Rotation/Scale/Identity`, plus `math.pi`, `math.radians`, `math.degrees`, `math.sqrt`, `math.sin`, `math.cos`, `math.tan`. Euler matrices use Blender's `eul_to_mat3` (`XYZ` = Rz·Ry·Rx), never `glam::EulerRot` [RUST §2].

### 7.6 Node trees

```rust
pub struct NodeTree { pub ty: NodeTreeType, pub nodes: Vec<Node>, pub links: Vec<NodeLink>,
                      pub interface: Vec<InterfaceSocket>, pub next_node_uid: u32 }
pub struct Node { pub uid: u32, pub name: SubName, pub label: String, pub bl_idname: &'static str,
                  pub location: [f32; 2], pub width: f32, pub parent: Option<u32>, pub mute: bool,
                  pub inputs: Vec<Socket>, pub outputs: Vec<Socket>, pub props: NodeProps /* per type */ }
pub struct Socket { pub name: &'static str, pub identifier: &'static str, pub ty: SocketType,
                    pub enabled: bool, pub hide_value: bool, pub default_value: SocketValue }
```

`nodes.new(type="ShaderNodeTexImage")` builds the node from `NodeTypeDef` (sockets, defaults, width 240, name = label uniquified). `links.new(from_output, to_input)` replaces an existing link into a single-input socket (verified). Removed 5.x nodes (`ShaderNodeSeparateRGB`…) give Blender's `RuntimeError: Error: Node type ShaderNodeSeparateRGB undefined`. `materials.new()` always creates `Principled BSDF` at (−200,100) linked to `Material Output` at (200,100). The `glTF Material Output` node group is created on demand, exactly as the add-on does [GLTF §4.5].

---

## 8. Front-ends: parsers and value grammar

### 8.1 One IR, two syntaxes

`frap-lang` is a hand-written lexer plus recursive descent (Pratt for arithmetic) with zero dependencies [RUST §7]. Both syntaxes lower to the same IR. The IR knows nothing about operators or RNA; resolution happens in `frap-wm`.

```rust
// crates/frap-lang/src/ir.rs
pub struct Stmt { pub kind: StmtKind, pub span: Span, pub src_line: u32 }
pub enum StmtKind {
    OpCall { idname: String, args: Vec<Arg> },                 // bpy.ops.mesh.x(...) / mesh.x k=v
    Call   { target: PathExpr, func: String, args: Vec<Arg> }, // RNA function / coll API
    Assign { target: PathExpr, op: AssignOp /* = += -= *= /= */, value: Expr },
    Eval   { expr: Expr, print: bool },                        // bare expr (REPL repr) or print(expr)
    Noop   { reason: NoopReason },                             // import bpy, from mathutils import Vector, pass
}
pub struct Arg { pub name: Option<String>, pub value: Expr, pub span: Span }
pub enum Expr { Lit(Literal), Path(PathExpr), Tuple(Vec<Expr>), List(Vec<Expr>), Set(Vec<Expr>),
                Dict(Vec<(Expr, Expr)>), Unary(UnOp, Box<Expr>), Binary(BinOp, Box<Expr>, Box<Expr>),
                Call { callee: PathExpr, args: Vec<Arg> },     // Vector(..), math.radians(..), coll.get(..)
                Bare(String) /* short form only: untyped token, typed by target PropDef */ }
pub struct PathExpr { pub root: Root, pub segs: Vec<PathSeg> }  // shared with frap-rna::RnaPath
```

`Session::exec_stmt(&Stmt) -> Response` is the only entry point, and a future `mlua` front-end builds the same `Stmt`s (or calls `call_operator`, `rna_get`, `rna_set` and `rna_call` directly).

### 8.2 bpy-line syntax (a Python subset, line oriented)

- Statements are separated by newline or `;`. A statement continues across lines while brackets are open. `#` starts a comment.
- Allowed: `bpy.ops.<ns>.<op>(kw=…)`, path assignment and augmented assignment, path function calls, bare expressions, `print(expr)`, and `import`/`from … import …` (no-ops limited to `bpy`, `math`, `mathutils`, `bmesh`).
- Literals: Python ints/floats (with `_`, exponent, `inf`/`nan` via `float('inf')`), strings in `'…'`, `"…"`, `r'…'` with escapes, `True/False/None`, tuples (including `(x,)`), lists, sets (`{'VERT'}`), dicts (macro sub-props).
- Rejected with `SyntaxError` plus a frap explanation: name binding (`obj = …`; D9), `for/while/if/def/class/with/lambda`, comprehensions, and attribute calls on non-path receivers.
- Python's own `SyntaxError` wording for real syntax errors (`'(' was never closed`) is kept where cheap.

### 8.3 Short form

```
line       := op_call | path_stmt
op_call    := NS '.' OPNAME (WS kwarg)*                       # mesh.primitive_cube_add size=2 location=0,0,1
kwarg      := key '=' svalue
key        := IDENT ('.' IDENT)*                              # TRANSFORM_OT_translate.value=0,0,1  (macro)
path_stmt  := spath (WS? ('='|'+='|'-='|'*='|'/=') WS? svalue)?   # get if no '='
            | spath '.' FUNC (WS kwarg | WS svalue)*          # data.objects['Cube'].select_set True
spath      := ('data'|'context') accessor*                    # same accessors as 7.4
svalue     := item (',' item)*                                # comma tuple, no spaces: 0,0,1
            | '(' <bpy-expr> ')'                              # escape hatch: full expression grammar
item       := NUMBER | 'True'|'False'|'true'|'false' | STRING | '{' item (',' item)* '}' | BARE | spath
BARE       := [A-Za-z_][A-Za-z0-9_.\-]*                       # enum id or name, typed by the target prop
```

- **Disambiguation**: a line starting with `bpy.`, `print(`, `import ` or `from ` is bpy-line. Otherwise it is short form. `data`/`context` are reserved roots, and no operator namespace uses those names.
- **Typed coercion**: bare tokens are typed by the target `PropDef`: `type=FACE` → enum, `name=Cube.001` → string, `size=2` → float. That means `{VERT,EDGE}` and `delimit=SEAM,UV` both work for enum-flag sets. Argument errors use the same Blender messages as the bpy form.
- Positional values are allowed only for RNA function calls, mapped in FuncDef order. Operators are keyword-only, as in Blender.

### 8.4 Examples

```
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))
mesh.primitive_cube_add size=2 location=0,0,1

bpy.data.objects['Cube'].location = (1, 2, 3)
data.objects['Cube'].location = 1,2,3

bpy.context.object.modifiers['Bevel'].width = 0.1
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, 1)})
mesh.extrude_region_move TRANSFORM_OT_translate.value=0,0,1

bpy.data.materials['Mat'].node_tree.links.new(bpy.data.materials['Mat'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Mat'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.context.object.keyframe_insert("location", frame=10)
bpy.context.object.vertex_groups['Bone'].add([0, 1, 2], 0.5, 'REPLACE')
bpy.context.object.matrix_world          # bare expr → value
```

---

## 9. JSON response protocol

One JSON object per statement on stdout, newline-delimited. Logs go to stderr (`tracing`). The first line is a ready message: `{"type":"ready","frap":"0.1.0","blender_api":"5.2.4","protocol":1,"scene":"factory"}` (disable with `--no-ready`).

### 9.1 Success: operator

```json
{"type":"result","line":4,"stmt":0,"kind":"op","op":"mesh.extrude_region_move",
 "status":"FINISHED","return":["FINISHED"],
 "reports":[],
 "changes":{"created":[],"removed":[],"renamed":[],"modified":["bpy.data.meshes['Cube']"]},
 "context":{"mode":"EDIT_MESH","active_object":"Cube","selected_objects":["Cube"],"frame":1},
 "edit_mesh":{"Cube":{"verts":12,"edges":20,"faces":10,"select_mode":["VERT"],
              "selected":{"verts":4,"edges":4,"faces":1},"active":{"type":"FACE","index":6}}},
 "summary":{},
 "hints":[]}
```

- `status` and `return` use Blender's set names (`FINISHED`, `CANCELLED`). `reports` are Blender's `{type: INFO|WARNING|ERROR|OPERATOR, message}`, wording copied (`Removed 6 vertice(s)`, `Applied modifier was not first, result may not be as expected`).
- `changes` is computed exactly by comparing Arc pointers against the transaction snapshot (§15). It lists IDs as reusable `repr` paths.
- `summary` is op-specific. Primitives and `*_add` give `{"object": "bpy.data.objects['Cube.001']","data": "bpy.data.meshes['Cube.001']"}`. `modifier_add` gives the modifier name and path. `remove_doubles` gives `{"removed": N}`. `render.render` gives the written files and the engine. `export_scene.gltf` gives byte counts and the extension lists.
- `context` and `edit_mesh` are always present at `--verbosity=normal` (the default), so an agent never has to poll state. `--verbosity=minimal` drops them, and `full` adds modifier status and evaluated counts.

### 9.2 Success: get, set, call

```json
{"type":"result","line":7,"kind":"get","status":"OK","path":"bpy.data.objects['Cube'].location",
 "value":[1,2,3],"rna":{"type":"FLOAT","subtype":"TRANSLATION","array":[3],"py":"Vector"},"hints":[]}
{"type":"result","line":8,"kind":"set","status":"OK","path":"bpy.data.objects['Cube'].location","value":[1,2,3],
 "changes":{"modified":["bpy.data.objects['Cube']"]}}
{"type":"result","line":9,"kind":"call","status":"OK","path":"bpy.data.materials","func":"new",
 "value":{"$ref":"bpy.data.materials['Material.001']"},"changes":{"created":["bpy.data.materials['Material.001']"]}}
```

Value encoding [RNA §1.3]: arrays are plain JSON arrays, with the mathutils type in `rna.py` (`Quaternion` is w first, axis-angle is angle first, `Matrix` is row-major rows). Enum sets are sorted arrays. ID pointers are `{"$ref": "<repr>"}`. Non-ID structs are `{"$ptr": "<repr>"}`. Collections give `{"$coll": "<repr>", "len": N, "names": [...]}` (names capped at 100). Floats use shortest f32 form (D8). Non-finite floats are `{"$float":"nan"}`, `"inf"` or `"-inf"`. A read of a stale derived value (D3/D4) adds `hints: [{"code":"STALE_EVAL","message":"matrix_world is not re-evaluated until the next operator, bpy.context.view_layer.update() or frame_set (Blender behaviour)"}]`.

### 9.3 Errors and cancellation

```json
{"type":"result","line":12,"kind":"op","op":"mesh.primitive_cube_add","status":"ERROR",
 "error":{"exception":"TypeError","code":"OP_ARG_UNKNOWN",
          "message":"Converting py args to operator properties:: keyword \"sizee\" unrecognized",
          "detail":{"operator":"mesh.primitive_cube_add","argument":"sizee","did_you_mean":["size"],
                    "valid":["size","calc_uvs","enter_editmode","align","location","rotation","scale"]},
          "span":{"line":12,"col":33,"len":5}},
 "state":"unchanged","reports":[]}
{"type":"result","line":13,"kind":"op","op":"object.join","status":"CANCELLED","return":["CANCELLED"],
 "reports":[{"type":"WARNING","message":"Active object is not a selected mesh"}],"state":"unchanged"}
```

`exception` is the Python class Blender would raise. `code` is a stable frap code (§15.1). `message` is Blender's text. Everything else is frap help. `state` is always `unchanged` for ERROR and CANCELLED (§15).

---

## 10. Schema export (`frap --describe`)

Everything comes from `frap-spec` tables plus the overlay, so the schema cannot drift from the implementation.

```
frap --describe                       # full manifest (JSON)
frap --describe=jsonschema            # JSON Schema 2020-12 per operator input + protocol envelope
frap --describe=tools                 # LLM tool definitions (flattened: no $ref, enums inline, sets → array+uniqueItems)
frap --describe op:mesh.bevel         # single operator
frap --describe struct:BevelModifier  # single struct
frap --describe --implemented-only
```

Manifest shape (names follow Blender RNA metadata):

```json
{"frap_version":"0.1.0","blender_api":"5.2.4","protocol":1,
 "operators":[{"idname":"mesh.bevel","label":"Bevel","description":"…","options":["REGISTER","UNDO","BLOCKING","GRAB_CURSOR_XY"],
   "poll":{"modes":["EDIT_MESH"],"requires":["selection"],"matrix":{"A":"N","C":"Y"}},
   "properties":[{"identifier":"offset","type":"FLOAT","subtype":"DISTANCE","unit":"LENGTH","default":0.0,
     "hard_min":0,"hard_max":1e6,"soft_min":0,"soft_max":100,"hidden":false,"skip_save":false,
     "frap_note":"default 0 is a no-op; pass offset"}],
   "frap":{"status":"implemented","fidelity":"T2","headless":"blender","notes":[]}}],
 "structs":[{"identifier":"Object","base":"ID","properties":[…],"functions":[…]}],
 "python_only":[…],"enums":{"render.engine":["BLENDER_EEVEE","BLENDER_WORKBENCH","CYCLES"]},
 "protocol":{"$schema":"https://json-schema.org/draft/2020-12/schema", …}}
```

Hidden props are always included and flagged [MESH §0.9]. `is_set`-sensitive props carry `"unset_semantics"` (for example `location`: "omitted = 3D cursor"). Unimplemented operators appear with `status:"not_implemented"`, so an agent can tell "doesn't exist in Blender" from "not yet in frap". `schemars` only generates the static envelope and error schemas [RUST §9].

---

## 11. `.frap` project format

A zip container written by `wm.save_as_mainfile(filepath='x.frap')` / `wm.save_mainfile`, read by `wm.open_mainfile`. A `.blend` path gives `RuntimeError: Error: frap cannot write .blend files; use a .frap path` (code `IO_UNSUPPORTED_FORMAT`).

```
x.frap (zip, deterministic)
├─ frap.json              {"format":"frap","format_version":1,"blender_api":"5.2","frap_version":"0.1.0",
│                          "generator":"frap 0.1.0"}
├─ scene.json             all IDs + WindowManager state, pretty-printed, sorted, diffable
├─ buffers/<sha256>.bin   little-endian attribute columns, face offsets, deform verts, keyframes > 64
└─ images/<sha256>.<ext>  original encoded bytes of packed images (PNG/JPEG/WebP/EXR/HDR/KTX2)
```

`scene.json` excerpt:

```json
{"ids":{
  "objects":[{"name":"Cube","session_uid":3,"type":"MESH","data":{"$id":"meshes/Cube"},
     "location":[0,0,0],"rotation_mode":"XYZ","rotation_euler":[0,0,0],"rotation_quaternion":[1,0,0,0],
     "scale":[1,1,1],"parent":null,"matrix_parent_inverse":[[1,0,0,0],[0,1,0,0],[0,0,1,0],[0,0,0,1]],
     "mode":"OBJECT","modifiers":[{"name":"Bevel","type":"BEVEL","persistent_uid":1,"width":0.1, "…":"…"}],
     "material_slots":[{"link":"DATA","material":null}]}],
  "meshes":[{"name":"Cube","counts":{"verts":8,"edges":12,"faces":6,"corners":24},
     "face_offsets":{"$buf":"buffers/5e1c….bin","type":"i32","len":7},
     "attributes":[{"name":"position","domain":"POINT","data_type":"FLOAT_VECTOR","$buf":"buffers/9a0f….bin"},
                   {"name":"sharp_face","domain":"FACE","data_type":"BOOLEAN","$buf":"buffers/…"}],
     "materials":[{"$id":"materials/Material"}]}]},
 "scenes":[{"name":"Scene","view_layers":[{"name":"ViewLayer","bases":[{"object":"Cube","select":true}],"active":"Cube"}],"…":"…"}],
 "window_manager":{"scene":"Scene","view_layer":"ViewLayer","view3d":{"…":"…"}}}
```

Rules:
- Properties are keyed by **RNA identifier**. All values for implemented props are written, not just non-defaults, so diffs stay stable when defaults change.
- **Orphan purge on save**: IDs with `users == 0` and no fake user are dropped, as Blender does [OBJ §1.11]. The in-memory state is unchanged unless the agent reloads.
- Objects in edit mode: the edit mesh is flushed to the Mesh before writing (Blender does this on save), `mode:"EDIT"` is recorded, and edit mode is re-entered on open. Select history is not saved.
- `bpy.data.filepath` and `is_dirty` update like Blender (`Saved "x.frap"` INFO report). `copy=True` leaves `filepath` unchanged. `//` paths resolve relative to the .frap location.
- **Determinism**: entries are sorted (`frap.json`, `scene.json`, `buffers/*`, `images/*`), the timestamp is fixed at 1980-01-01, JSON and `.bin` are Deflated at a fixed level, images are Stored. Saving the same state twice gives byte-identical files (test).
- **Versioning**: `format_version` is an integer. `frap-project/src/versioning/v{N}_to_v{N+1}.rs` migrates `serde_json::Value` trees (the counterpart of Blender's `versioning_*.cc`). Newer versions are refused with a clear error. `blender_api` is recorded so that 5.3 RNA renames can be migrated.

---

## 12. glTF/GLB

### 12.1 `frap-gltf`: the spec model (Blender-agnostic)

```rust
pub struct Gltf { pub asset: Asset, pub extensions_used: Vec<String>, pub extensions_required: Vec<String>,
    pub scene: Option<u32>, pub scenes: Vec<Scene>, pub nodes: Vec<Node>, pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>, pub textures: Vec<Texture>, pub images: Vec<Image>,
    pub samplers: Vec<Sampler>, pub accessors: Vec<Accessor>, pub buffer_views: Vec<BufferView>,
    pub buffers: Vec<Buffer>, pub skins: Vec<Skin>, pub cameras: Vec<Camera>,
    pub animations: Vec<Animation>,
    pub extensions: IndexMap<String, serde_json::Value>, pub extras: Option<Box<RawValue>> }
pub struct GlbFile { pub json: Gltf, pub bin: Option<Vec<u8>> }
pub fn read_glb(bytes: &[u8]) -> Result<GlbFile, GltfError>;
pub fn write_glb(f: &GlbFile, opts: &JsonWriteOpts /* blender_compat key order & float fixups */) -> Vec<u8>;
pub mod accessor { pub fn decode<T: Element>(g: &Gltf, bin: &[u8], idx: u32) -> Result<Vec<T>, GltfError>; /* sparse, normalized, quantized */ }
pub trait MeshCompressor { fn encode(&self, prim: &PrimitiveData, q: &Quantization) -> Result<Vec<u8>, CodecError>;
                           fn decode(&self, bytes: &[u8], attrs: &AttrMap) -> Result<PrimitiveData, CodecError>; }
```

The serde model is hand-written, every extension known to the 5.2 add-on is typed, and unknown extensions and extras round-trip losslessly [RUST §1]. Codecs sit behind features: `draco` (`draco-core` spike, `=2.3.3`), `meshopt` (`meshopt 0.7`), `webp-lossy` (`webp`), `ktx2` (read: `ktx2` + `basisu`; encode: optional `basisu-encode`).

### 12.2 `frap-io-gltf`: a port of `io_scene_gltf2` 5.2.41 (Apache-2.0)

The fidelity strategy is a **module-by-module port of the add-on**, keeping its file structure. The license is compatible under either D1 choice.

| add-on module | frap module |
|---|---|
| `blender/exp/tree.py` (vtree, filters, natural sort) | `exp/vtree.rs` |
| `blender/exp/primitive_extract.py` | `exp/primitive_extract.rs` (looptris, dot keys, byte-sort unique, 4-dp normals) |
| `blender/exp/material/search_node_tree.py`, `material/*.py`, `extensions/*.py` | `exp/material/…` |
| `blender/exp/image.py` (ORM packing, passthrough) | `exp/image.rs` (uses `frap-imbuf`) |
| `blender/exp/animation/*` (sampling, fcurves, NLA modes) | `exp/animation/…` (uses `frap-depsgraph::frame_set`) |
| `blender/exp/skins.py`, `joints.py`, `lights.py`, `cameras.py` | same names |
| `io/exp/export.py` (GLB writer, key order, `__fix_json`) | `frap-gltf::write_glb(blender_compat)` |
| `blender/imp/*` | `imp/…` |

- `ExportSceneGltf` props are generated from the 110 dumped props with identical names and defaults (`export_apply=False`, `export_cameras=False`, `export_lights=False`, `export_yup=True`, `export_force_sampling=True`, …). frap-only options are namespaced: `frap_texture_format='KTX2'`, `frap_interleave=True`, `frap_quantize=…`, `frap_node_visibility=True`, `frap_fps_base_fix=True`. With every `frap_*` option at default, **output is Blender-compat**: non-interleaved views, gather order, omission rules, rest at frame 0, fake white COLOR_0, BYTE_COLOR → USHORT, `extensionsRequired` for lights and texture_transform, and the fps_base time formula (flagged by a WARNING report when `fps_base != 1`).
- `export_use_gltfpack=True` maps the `export_gltfpack_*` props onto in-process meshopt with gltfpack-equivalent semantics, so no external binary is needed.
- The exporter reads frap's typed DNA directly (not RNA strings) but walks the same graph. Material detection runs on frap node trees using the add-on's patterns.
- Import is the inverse port: merge rules, `import_shading` from normals, the BLENDER bone heuristic, `guess_original_bind_pose`, NLA stashing, material graph rebuild including the `glTF Material Output` group and `Volume Absorption`. A GLB → frap → GLB round-trip must be equal modulo vertex order (golden).

---

## 13. Renderer

### 13.1 Structure

```
frap-render/src/
  lib.rs            RenderEngine trait, engine registry, render.render op impl
  gpu.rs            wgpu instance/adapter/device; WGPU_BACKEND/WGPU_ADAPTER_NAME; error if none
  extract/          RenderScene from Depsgraph (CPU, deterministic): batches, materials, lights, camera, world
  material/         shader-node graph → WGSL codegen (GPUMaterial-like) for the supported node set
  eevee/            prepass, shadows, forward lighting, transparency, world/IBL, TAA, film
  workbench/        studio/matcap/flat lighting, color types, xray, wireframe, outline, shadows
  colormanagement/  view transform LUTs, look, exposure, gamma, display encode, dither
  output.rs         frame_path rules, file formats via frap-imbuf, "Render Result" image
  contact_sheet.rs  frap.render_contact_sheet
```

```rust
pub trait RenderEngine {
    const ID: &'static str;                       // "BLENDER_EEVEE" | "BLENDER_WORKBENCH"
    fn render_frame(&mut self, gpu: &Gpu, scene: &RenderScene, s: &RenderSettingsSnapshot)
        -> Result<RenderResult, RenderError>;
}
pub struct RenderResult { pub width: u32, pub height: u32,
                          pub combined: Vec<[f32; 4]> /* scene-linear, straight or premul per film_transparent */ }
pub struct RenderScene { pub camera: CameraParams, pub batches: Vec<DrawBatch>, pub materials: Vec<GpuMaterialDesc>,
                         pub lights: Vec<LightDesc>, pub world: WorldDesc, pub frame: f32 }
```

### 13.2 Pipeline for `render.render(write_still=True)`

1. `ensure_evaluated`. Without `scene.camera`: `RuntimeError: Error: Cannot render, no camera`.
2. Extract the RenderScene from evaluated meshes (looptris, corner normals, UVs, colour attributes, MikkTSpace tangents when a normal map is used) and the camera (`BKE_camera_params` port: sensor fit, shift, clip, `resolution_*` × percentage, pixel aspect).
3. Run the engine into an `Rgba16Float` target. Read back with 256-byte row padding stripped [RUST §3].
4. **Color management** on the CPU with `spec/blender-5.2/colormanagement/*.cube`. These LUTs are dumped from Blender by rendering a colour lattice EXR through `image.save_render()` with each view transform. They cover Standard, AgX (default), Filmic, Khronos PBR Neutral and False Color, with `look`, `exposure` and `gamma` applied as in OCIO's processor order. Output is display sRGB, with dithering per `render.dither_intensity`.
5. Store as `bpy.data.images['Render Result']`. With `write_still`, write to `render.filepath` with Blender's rules (extension added when `use_file_extension`, `//` relative), in `image_settings.file_format/color_mode/color_depth/compression/quality`.
6. `animation=True`: for each frame in `frame_start..=frame_end` step `frame_step` (op `frame_start`/`frame_end` override only when `is_set`), run `frame_set`, then render and write `frame_path(frame)` (`#` runs zero-padded, otherwise a 4-digit suffix).

### 13.3 EEVEE-like engine

- **Materials**: the shader-node subset the glTF exporter understands, compiled to WGSL. Nodes covered: Principled BSDF (base, metallic, roughness, IOR, alpha, normal, specular IOR level/tint, coat, sheen, emission, transmission (screen-space approximation), anisotropy (approximation)), Image Texture (interpolation, extension, alpha), Normal Map (tangent, OpenGL/DirectX `convention`), Mapping, UV Map, Texture Coordinate, Color Attribute, Mix (all data types), Math, Separate/Combine Color, Emission, Transparent, Mix/Add Shader, Background, Environment Texture. An unsupported node evaluates its default socket values and adds a WARNING report naming it.
- **Lights**: Blender units (W, W/m², `spot_size`, `spot_blend`, `exposure`, `temperature`, `normalize`, `shadow_soft_size`) converted to radiance with EEVEE's own power normalisation (a port of `eevee_light` power functions; T4 golden on a single-light scene). Shadows: cube maps for point/spot and cascades for sun (`Depth32Float`, PCF filtered by `shadow_soft_size`).
- **World**: a constant colour, or an HDRI (Environment Texture → Background) with split-sum IBL (irradiance SH, prefiltered specular mips, BRDF LUT computed on the GPU at world load). `film_transparent` gives alpha.
- **Transparency**: `surface_render_method` DITHERED (hashed) or BLENDED (sorted per object), following EEVEE.
- **AA**: `scene.eevee.taa_render_samples` jittered accumulation with a fixed Halton sequence.

### 13.4 Workbench engine

Reads `scene.display.shading`: `light` STUDIO|MATCAP|FLAT, `color_type` MATERIAL|OBJECT|RANDOM|VERTEX|TEXTURE|SINGLE, `show_xray`/`xray_alpha`, `show_shadows`, `show_cavity` (P2), `show_object_outline`, `background_type`, `wireframe_color_type`, plus `scene.display.light_direction` and `shadow_shift`. `RANDOM` colours use Blender's hash of the object name, so they match. Studio lights and matcaps are captured from Blender as fixtures (licensing note in §18). WIREFRAME is a shading type here, so `render.render` can produce wireframe and x-ray images, as viewport shading does.

### 13.5 Contact sheet (`frap.render_contact_sheet`)

```
bpy.ops.frap.render_contact_sheet(filepath="//sheet.png", views={'FRONT','RIGHT','TOP','CAMERA','ISO'},
    columns=0, view_size=512, engine='BLENDER_WORKBENCH', shading='SOLID', use_frame_selected=False,
    margin=0.1, show_labels=True, frame=None)
```

- Axis views are orthographic, framed on the evaluated bounds of visible objects (or selected ones). The views use Blender's numpad orientations: FRONT is −Y looking +Y, RIGHT is +X, TOP is +Z. `CAMERA` uses `scene.camera`, and `ISO` uses the factory viewport rotation.
- Temporary cameras are **not** added to `Main`.
- Labels use an embedded bitmap font. The summary lists the cell layout so an agent can map cells back to views.

---

## 14. Golden test harness against Blender

### 14.1 Case format

```python
# tests/golden/cases/mesh/extrude_top.py
# frap-golden: v1
# start: factory
# compare: lines; mesh:Cube[tier=T1]; selection:Cube; objects
# tol: 1e-5
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])       # frap op: emulated on the Blender side
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, 1)})
bpy.ops.mesh.primitive_cube_add(sizee=2)                      # expected TypeError on both sides
```

Cases are written in the **bpy-line subset** that both tools accept. Short-form parity is checked separately by a transpile test (every case → short form → same IR).

### 14.2 Runners

```rust
// crates/frap-golden/src/lib.rs
pub struct Case { pub path: PathBuf, pub start: Start, pub compare: Vec<CompareSpec>, pub tol: f32 }
pub struct Snapshot { pub lines: Vec<LineOutcome>, pub state: SceneSnapshot }   // same JSON from both sides
pub struct LineOutcome { pub line: u32, pub ok: bool, pub exception: Option<String>, pub message: Option<String>,
                         pub value: Option<serde_json::Value> }
pub fn run_frap(case: &Case) -> Snapshot;                              // in-process Session
pub fn run_blender(case: &Case, blender: &Path) -> Result<Snapshot, HarnessError>;
pub fn compare(frap: &Snapshot, blender: &Snapshot, spec: &[CompareSpec]) -> Vec<Mismatch>;
pub fn locate_blender() -> Option<PathBuf>; // $BLENDER_BIN, PATH, ~/Applications/Blender-5.2.app/…
```

- **Blender side** (`tools/blender/golden_runner.py`): `blender -b --factory-startup --python golden_runner.py -- case.py out.json`. It executes each statement with `exec` inside `bpy.context.temp_override(window, area=VIEW_3D, region=WINDOW)` of the Layout screen [OBJ §1.7], records the exception class and message per line, then calls `snapshot.py`. Objects in edit mode get `update_from_editmode()` first.
- **frap side**: the same statements through `Session`, then a snapshot through **frap's own RNA** using the property list in `tests/golden/props.toml`, which `snapshot.py` also reads. The snapshot therefore also tests RNA paths.
- **Snapshot content**: per-collection ID names in `bpy.data` order; listed props per ID; meshes as `vertices.co`, `edges.vertices`, `polygons.vertices`, `loop_start`, every attribute (name/domain/type/data), UV layers, vertex groups, shape keys; edit-mode select state, history and active element; base order and selection; F-curves (data_path, index, keyframe co, handles, interpolation); modifier stacks; evaluated mesh (`evaluated_get` + `to_mesh`) when requested.

### 14.3 Comparators by tier

| Tier | Comparator |
|---|---|
| T0 | line outcomes: same `ok`, same exception class, normalised message equality; RNA values exact (floats within tol) |
| T1 | arrays compared index by index (positions within tol); topology exact; selection exact |
| T2 | canonical form: sorted position multiset; faces as rotation-normalised position tuples; per-corner attributes keyed by (face canonical id, vertex position) |
| T3 | invariants listed in the case (UV bounds, no overlap, area ratio, manifold, Euler characteristic) |
| T4 | images: SSIM ≥ threshold (Workbench 0.98, EEVEE 0.90 at first), plus mean ΔE in a centre crop |
| GLB | decode both files; JSON structure normalised by names; accessors decoded with tol; geometry as a canonical multiset of triangles of attribute tuples; images by pixels; ignore `asset.generator` [GLTF §7.2] |

### 14.4 Emulations, fixtures, drift

- `tools/blender/emulations.py` maps frap-extended or frap-namespaced ops to Blender equivalents: `mesh.loopcut(edge_index=…)` → `bmesh.ops.subdivide_edges` recipe + `select_edge_ring_multi` [MESH App. B.7]; `frap.select_elements` → bmesh selection; `mesh.spin` with axis → the same op; `frap.render_contact_sheet` → skipped.
- **Fixtures**: Blender snapshots are committed at `tests/golden/fixtures/<case>.json`, stamped with the Blender build hash. CI without Blender compares against the fixtures. `cargo xtask golden --bless` regenerates them. With Blender present, the harness also re-runs Blender and fails on fixture drift.
- **Spec drift**: `cargo xtask spec-refresh --check` re-dumps `spec/blender-5.2/` and fails on any diff. It runs on every Blender patch release.
- **Differential fuzzing** (M8): a generator draws random sequences of implemented P0 ops with valid props and runs both tools. Mismatches are minimised automatically and saved as cases.
- **Coverage report**: `xtask golden --report` gives an op × tier matrix, published in CI.
- Missing Blender: tests that need a live Blender are `#[ignore]`d with a visible notice. Fixture comparisons still run.

---

## 15. Error handling and transactional rollback

### 15.1 Error taxonomy

```rust
// crates/frap-proto/src/error.rs
pub enum PyExc { SyntaxError, TypeError, ValueError, KeyError, IndexError, AttributeError,
                 RuntimeError, NotImplementedError }
pub struct FrapError {
    pub exception: PyExc, pub code: ErrorCode, pub message: String,  // Blender wording
    pub detail: serde_json::Map<String, serde_json::Value>,          // did_you_mean, valid items, path…
    pub span: Option<Span>, pub reports: Vec<Report>,
}
pub enum ErrorCode { SYNTAX, UNSUPPORTED_SYNTAX, OP_UNKNOWN, OP_NOT_IMPLEMENTED, OP_ARG_UNKNOWN,
    OP_ARG_TYPE, OP_ARG_ENUM, OP_ARG_LENGTH, OP_POLL_FAILED, OP_EXEC_ERROR, OP_CANCELLED,
    RNA_PATH_NOT_FOUND, RNA_KEY_NOT_FOUND, RNA_INDEX_RANGE, RNA_READONLY, RNA_TYPE, RNA_ENUM,
    RNA_FUNC_ARGS, IO_ERROR, IO_UNSUPPORTED_FORMAT, GPU_UNAVAILABLE, RENDER_NO_CAMERA, INTERNAL }
```

### 15.2 Transactions

Every statement (op, set or call) runs in a transaction.
- `Main` holds IDs as `Arc<T>`, so `Main::snapshot()` clones the ID tables in O(#IDs) without copying data. The first mutable access to an ID in the transaction runs `Arc::make_mut` and copies only that ID. Large mesh arrays inside a copied Mesh stay shared until written (`Shared<[T]>`). The edit mesh (`Arc<BMEditMesh>`) is copied on the first write in the statement. This matches the cost of Blender's own edit-mesh undo, which stores mesh copies.
- `WindowManager` and `Depsgraph` tags are cloned too (small).
- On ERROR or CANCELLED (and on a caught panic), the snapshot is restored, depsgraph entries whose `IdList::version` no longer matches are invalidated, and the response says `"state":"unchanged"`.
- On success, `changes` comes from comparing each ID's Arc pointer with the snapshot (modified), plus the slot tables (created and removed) and the name tables (renamed).
- Side effects outside the scene (files written by `render.render`, `export_scene.gltf` or `save_as_mainfile`) are not rolled back. Writers stream to a temp file and atomically rename it on success, so a failed op never leaves a partial file.

### 15.3 Robustness

- Each statement runs inside `catch_unwind`. A panic becomes `RuntimeError` with code `INTERNAL`, a rollback, and the stream continues. Release builds use `panic=unwind`.
- Invalid UTF-8 on stdin → `SyntaxError` for that line. EOF → exit 0. `--fail-fast` exits non-zero on the first error (for CI scripts).
- Limits: `--max-elements` (default 50M corners) and `--stmt-timeout` (cooperative checks in long loops such as subdivide, boolean and render) give `RuntimeError: Error: frap: limit exceeded`, with rollback.

---

## 16. Determinism

- **No hash-order output**: `IndexMap`/`BTreeMap` everywhere a collection is serialized, iterated into results or used to build geometry. `rustc-hash` maps are used only for lookups.
- **Session counters** (`session_uid`, node uid, modifier `persistent_uid`) start fixed per session, so responses are reproducible.
- **Random numbers** come from ports of Blender's generators: `BLI_rng` (drand48-style LCG) for `select_random`, `vertex_random` and `randomize_transform`, and `BLI_hash_int_*` for Workbench random colours and the subdivide fractal. Seeds are the operator's `seed` props.
- **Floating point**: f32 storage, f64 only where Blender uses double. No `fast-math`. Explicit `mul_add` only where Blender's source does it. Bit-identical results with Blender are **not** promised: Blender's arm64 builds may contract to FMA. Goldens use tolerances (T1 1e-5, bevel and UV solvers 1e-4).
- **Parallelism**: rayon only for order-independent work (per-element normals, per-object evaluation with a deterministic merge). Reductions use fixed-order pairwise sums.
- **Files**: `.frap` is byte-reproducible (§11). GLB output is byte-reproducible for the same frap state (a test).
- **Render**: fixed TAA sequence and fixed shadow filtering. Output varies across GPU vendors, which T4 tolerances absorb. CI uses lavapipe (Linux) and WARP (Windows) [RUST §3].
- **Time**: no wall clock in results unless `--timing` is passed.

---

## 17. Milestone plan

Each work package (WP) **owns** the files listed. Two WPs in the same milestone never own the same file, so they can be built in parallel. A shared file needs a **seam** owned by one WP. The seams are: `frap-wm/src/registry.rs` (each `frap-ed-*` crate registers itself, so no edits to wm), `frap-rna/src/structs/mod.rs` (generated list), and `frap-spec/src/generated/*` (only `xtask codegen` writes it). "Exit" criteria are golden cases passing at the declared tier.

### M0 — Spec and skeleton (foundation; everything depends on it)

| WP | Scope | Owns | Depends |
|---|---|---|---|
| 0.1 Spec dumps | `dump_ops/rna/nodes/modifiers/factory/luts.py`; commit `spec/blender-5.2/*`; overlay schema + initial overlay for P0 ops | `tools/blender/dump_*.py`, `spec/` | — |
| 0.2 Codegen | `xtask codegen`: OpDef/StructDef/PropDef/EnumDef/NodeTypeDef tables, typed `Props` structs, `from_args` with Blender messages | `xtask/src/codegen/`, `crates/frap-spec/` | 0.1 (format) |
| 0.3 Workspace & CI | root Cargo.toml, toolchain, CI matrix (macOS arm64, Linux + lavapipe, Windows + WARP), `cargo-deny`, license headers | `Cargo.toml`, `.github/`, `deny.toml` | — |
| 0.4 Math | mathutils semantics, Euler/quat/matrix decompose (`mat4_to_loc_rot_size`), compatible euler, `BLI_rng` | `crates/frap-math/` | — |
| 0.5 Language | lexer, bpy-line + short-form parsers, IR, transpile test, fuzzing (proptest) | `crates/frap-lang/` | — |
| 0.6 Protocol | envelope, `FrapError`, codes, schemars for static schemas, describe skeleton | `crates/frap-proto/` | 0.2 |
| 0.7 Golden harness skeleton | `golden_runner.py`, `snapshot.py`, `props.toml`, `frap-golden` comparators T0–T3, fixtures and bless | `tools/blender/golden_*.py`, `tools/blender/snapshot.py`, `crates/frap-golden/`, `tests/golden/` | 0.1 |

### M1 — Data model and object mode (first vertical slice)

| WP | Scope | Owns | Depends |
|---|---|---|---|
| 1.1 DNA core | Main, IdList (sorted, Arc COW), IdRef, naming, users, Object/Scene/ViewLayer/Base/Collection/Camera/Light/World/Material (tree stub), factory startup | `crates/frap-dna/` (except `mesh.rs`, `node_tree.rs`) | 0.2, 0.4 |
| 1.2 DNA mesh + geom | `Mesh`, AttributeSet, normals, looptris, polyfill2d + beautify, kdtree | `crates/frap-dna/src/mesh.rs`, `crates/frap-geom/` | 1.1 |
| 1.3 BMesh core | MemPool, elements, kernel Euler ops, CustomData, bm_from_me/bm_to_me, select flush, index ensure, primitive bmo ops (9 + monkey per D7) | `crates/frap-bmesh/` (`core/`, `mempool.rs`, `convert.rs`, `select.rs`, `ops/primitive.rs`) | 1.2 |
| 1.4 RNA runtime | ptr/resolve/get/set/call, value conversion and messages, `rna_id/object/scene/view_layer/collection/mesh(elements)/camera/light/world` | `crates/frap-rna/` | 1.1, 0.2 |
| 1.5 WM | Session, Context, Registry, invocation pipeline, macros, reports, transactions, virtual View3D | `crates/frap-wm/` | 1.1, 1.4 |
| 1.6 Depsgraph v1 | tags, relations, transform/parent eval, bounds, timing per §4.1, copy-back, frame_set stub | `crates/frap-depsgraph/` | 1.1 |
| 1.7 Object ops | mode_set, select_all/pattern/by_type, delete, duplicate(_move/_linked), join, parent_set/clear, origin_set, transform_apply, *_clear, shade_*, camera/light/empty_add, collection link/move, material slots (no node logic) | `crates/frap-ed-object/` | 1.5 |
| 1.8 Transform | transform.translate/rotate/resize/mirror for object mode + orientation/pivot core | `crates/frap-ed-transform/` | 1.5 |
| 1.9 Primitives ops | `mesh.primitive_*_add` (cursor/align/is_set rules, edit-mode merge) | `crates/frap-ed-mesh/src/primitives.rs` | 1.3, 1.5 |
| 1.10 CLI | stdin loop, verbosity, `--empty`, `--describe`, `--fail-fast` | `crates/frap-cli/` | 1.5, 0.5, 0.6 |
| 1.11 Project format | `.frap` save/open, orphan purge, deterministic zip, versioning scaffold | `crates/frap-project/` | 1.1 |

Exit: about 80 golden cases (object ops T1, primitives T1, RNA set/get T0, error classes T0), and `.frap` round-trip byte-identical.

### M2 — Edit-mode modelling

| WP | Scope | Owns | Depends |
|---|---|---|---|
| 2.1 Selection | select_all/mode/linked/more/less/non_manifold/loose/face_by_sides/edge_loop_multi/ring_multi/sharp, hide/reveal, frap.select_elements/select_by_normal | `frap-bmesh/src/ops/select_*.rs`, `frap-ed-mesh/src/select.rs`, `frap-ed-mesh/src/frap_ext.rs` | 1.3 |
| 2.2 Extrude & inset | extrude_* family + macros, extrude_repeat, inset (region/individual) | `frap-bmesh/src/ops/{extrude,inset}.rs`, `frap-ed-mesh/src/{extrude,inset}.rs` | 1.3 |
| 2.3 Subdivide & loops | subdivide, subdivide_edgering, unsubdivide, loopcut(_slide) headless, offset_edge_loops | `frap-bmesh/src/ops/subdivide*.rs`, `frap-ed-mesh/src/loopcut.rs`, `frap-ed-mesh/src/subdivide.rs` | 1.3 |
| 2.4 Topology edits | delete*, dissolve_*, merge, remove_doubles, edge_face_add, fill, fill_grid, bridge_edge_loops, vert_connect_path, split, edge_split, separate, duplicate(_move) | `frap-bmesh/src/ops/{delete,dissolve,weld,create,fill,bridge,connect,split,dupe}.rs`, `frap-ed-mesh/src/{delete,merge,create,split}.rs` | 1.3 |
| 2.5 Normals & marks | flip_normals, normals_make_consistent, faces_shade_*, mark_seam/sharp, set_sharpness_by_angle, custom normals add/clear, edge crease/bevel weight | `frap-bmesh/src/ops/normals.rs`, `frap-ed-mesh/src/{normals,marks}.rs` | 1.3 |
| 2.6 Tris/quads/misc | quads_convert_to_tris, tris_convert_to_quads, poke, symmetrize, spin, screw, vertices_smooth, solidify, wireframe, bisect | `frap-bmesh/src/ops/{triangulate,join_triangles,poke,symmetrize,spin,smooth,solidify,wireframe,bisect}.rs`, `frap-ed-mesh/src/misc.rs` | 1.3 |
| 2.7 Bevel | `bmesh_bevel` port (seg 1 → T1, then N segments, profiles, miters, harden normals) | `frap-bmesh/src/ops/bevel/`, `frap-ed-mesh/src/bevel.rs` | 1.3 |
| 2.8 Edit transform | edit-mode translate/rotate/resize, shrink_fatten, edge_slide, vert_slide, tosphere, pivots, NORMAL orientation, proportional | `frap-ed-transform/src/edit/` | 1.8, 1.3 |

Exit: the [MESH §5.8] P0 list at its declared tiers. Index-exact ops at T1.

### M3 — Materials, images, UVs, static GLB I/O (parallel with M2 after M1)

| WP | Scope | Owns | Depends |
|---|---|---|---|
| 3.1 Node trees | NodeTree DNA, NodeTypeDef from spec, socket availability, nodes.new/links.new, `rna_nodetree`, glTF Material Output group | `frap-dna/src/node_tree.rs`, `frap-rna/src/structs/rna_nodetree.rs`, `frap-dna/src/nodes/` | 1.1, 1.4 |
| 3.2 ImBuf | decode/encode PNG/JPEG/WebP/EXR/HDR/TGA/BMP, colorspace, packing, `images.new/load`, `image.pack/save/save_render` | `crates/frap-imbuf/`, `frap-rna/src/structs/rna_image.rs`, `frap-ed-object/src/image_ops.rs` | 1.1 |
| 3.3 glTF model | serde model, GLB container, accessor codec, compat JSON writer | `crates/frap-gltf/` (no codecs) | 0.3 |
| 3.4 GLB export static | vtree, primitive_extract, materials and all KHR_materials_*, texture_transform, images (ORM packing, passthrough), cameras, lights, extras, variants | `crates/frap-io-gltf/src/exp/` (except `animation/`, `skins.rs`) | 3.1–3.3, 1.2 |
| 3.5 GLB import static | meshes, materials graph rebuild, images, cameras, lights, hierarchy, naming | `crates/frap-io-gltf/src/imp/` (except `animation/`, `skin.rs`) | 3.1–3.3 |
| 3.6 UV | parametrizer LSCM (CONFORMAL first), ABF++, SLIM; smart_project; cube/cylinder/sphere_project; reset; pack_islands; average_islands_scale; seams_from_islands; follow_active_quads; lightmap_pack | `crates/frap-uv/`, `crates/frap-ed-uv/` | 1.3 |
| 3.7 Attribute & material ops | uv_texture_add/remove, color_attribute_add/remove, attribute_add/set, material.new, material_slot_*, world.new | `frap-ed-mesh/src/attributes.rs`, `frap-ed-object/src/material_ops.rs` | 1.7, 3.1 |

Exit: GLB golden suite for static content (canonical triangles, materials JSON structure); import round-trip.

### M4 — Modifiers and depsgraph v2

| WP | Scope | Owns | Depends |
|---|---|---|---|
| 4.1 Stack infra | ModifierTypeInfo, evaluation pipeline, modifier_add/remove/apply/copy/move*, `rna_modifier` (Appendix B), Smooth by Angle node group + `shade_auto_smooth`, `evaluated_get/to_mesh` | `crates/frap-modifiers/src/{lib,stack,nodes}.rs`, `frap-depsgraph/src/geometry.rs`, `frap-rna/src/structs/rna_modifier.rs`, `frap-ed-object/src/modifier_ops.rs` | 1.6, 1.3 |
| 4.2 Simple modifiers | MIRROR, ARRAY, SOLIDIFY, WELD, DISPLACE (+Texture IMAGE/NONE), TRIANGULATE, EDGE_SPLIT, WEIGHTED_NORMAL, SMOOTH, CAST, SIMPLE_DEFORM | `frap-modifiers/src/mod_{mirror,array,…}.rs` | 4.1 |
| 4.3 Subsurf | SUBSURF + `object.subdivision_set`; opensubdiv-rs spike or own Catmull-Clark; Blender layout ordering | `frap-modifiers/src/mod_subsurf/` | 4.1 |
| 4.4 Bevel/Decimate mods | BEVEL modifier on the 2.7 core; DECIMATE COLLAPSE/UNSUBDIV/DISSOLVE | `frap-modifiers/src/mod_{bevel,decimate}.rs`, `frap-bmesh/src/ops/decimate*.rs` | 4.1, 2.7 |
| 4.5 Boolean | MANIFOLD via manifold-rust, EXACT (T3), FLOAT plan; attribute transfer; `mesh.intersect(_boolean)` | `frap-modifiers/src/mod_boolean/`, `frap-ed-mesh/src/intersect.rs` | 4.1 |
| 4.6 Export with modifiers | `export_apply=True` path, linked-duplicate mesh sharing rules | `frap-io-gltf/src/exp/evaluated.rs` | 4.1, 3.4 |

### M5 — Rendering (can start after M1; materials need 3.1)

| WP | Scope | Owns | Depends |
|---|---|---|---|
| 5.1 Render core | GPU init, offscreen, readback, extract, render.render op, output rules, Render Result, file formats | `frap-render/src/{lib,gpu,extract,output}.rs` | 1.6, 3.2 |
| 5.2 Color management | LUT dumps + application, looks, exposure/gamma, dither | `frap-render/src/colormanagement/`, `tools/blender/dump_luts.py` | 5.1 |
| 5.3 Workbench | all shading/light/color types, xray, wireframe, outline, shadows; studio light fixtures | `frap-render/src/workbench/` | 5.1 |
| 5.4 EEVEE-like | material codegen, lights, shadows, IBL/HDRI, transparency, TAA | `frap-render/src/{eevee,material}/` | 5.1, 3.1 |
| 5.5 Animation render & contact sheet | `animation=True`, `frap.render_contact_sheet` | `frap-render/src/contact_sheet.rs`, `frap-render/src/animation.rs` | 5.1, 6.1 (frame_set) |
| 5.6 Render goldens | T4 cases, SSIM thresholds, CI software adapters | `tests/golden/cases/render/`, `frap-golden/src/image.rs` | 5.3, 5.4 |

### M6 — Animation, rigging, morphs

| WP | Scope | Owns | Depends |
|---|---|---|---|
| 6.1 Actions & F-curves | slotted actions DNA, keyframe_insert/delete, keyframe_points API, handle calc, F-curve eval, frame_set, screen.frame_*, anim.keyframe_* | `crates/frap-anim/`, `frap-dna/src/anim.rs`, `frap-rna/src/structs/rna_action.rs`, `frap-ed-anim/src/keyframes.rs` | 1.6 |
| 6.2 NLA | tracks, strips, blending/extrapolation eval | `frap-anim/src/nla.rs`, `frap-rna/src/structs/rna_nla.rs` | 6.1 |
| 6.3 Armatures | Armature/Bone/EditBone/PoseBone DNA+RNA, armature_add, edit-bone mode, pose mode, ARMATURE modifier, parent_set ARMATURE*, vertex groups API & ops | `frap-dna/src/armature.rs`, `frap-rna/src/structs/rna_armature.rs`, `frap-ed-anim/src/{armature,pose}.rs`, `frap-modifiers/src/mod_armature.rs`, `frap-ed-object/src/vgroup_ops.rs` | 4.1 |
| 6.4 Shape keys | Key DNA, shape_key_add/remove/clear, mixing, BMesh shape layers, ops | `frap-dna/src/key.rs`, `frap-depsgraph/src/key_eval.rs`, `frap-ed-object/src/shapekey_ops.rs` | 1.3, 1.6 |
| 6.5 glTF animation/skin/morph | export sampling + CUBICSPLINE path + modes + pointer; skins/IBM/neutral bone; morph targets + sparse; import counterparts | `frap-io-gltf/src/{exp,imp}/{animation,skins,morph}*` | 6.1–6.4, 3.4, 3.5 |

### M7 — Compression and spec-complete glTF

| WP | Scope | Owns |
|---|---|---|
| 7.1 Draco | `draco-core` spike behind `MeshCompressor`; fallback `-sys` plan | `frap-gltf/src/codec/draco.rs` |
| 7.2 meshopt & quantization | EXT/KHR meshopt, filters, KHR_mesh_quantization, gltfpack param mapping | `frap-gltf/src/codec/meshopt.rs`, `frap-io-gltf/src/exp/gltfpack.rs` |
| 7.3 Textures | EXT_texture_webp lossy; KTX2/BasisU read (+ optional encode) | `frap-gltf/src/codec/{webp,ktx2}.rs` |
| 7.4 Spec-complete writer | `frap_*` options: interleave, node.matrix, strips/fans, node_visibility, diffuse_transmission, UNSIGNED_BYTE indices | `frap-io-gltf/src/exp/spec_complete.rs` |
| 7.5 Instancing | EXT_mesh_gpu_instancing export/import | `frap-io-gltf/src/{exp,imp}/instancing.rs` |

### M8 — Fidelity hardening (continuous after M2)

8.1 differential fuzzer (`frap-golden/src/fuzz.rs`); 8.2 promote T2 → T1 ops (owners per op file); 8.3 P1/P2 operator breadth from the overlay backlog; 8.4 performance budgets (1M-face mesh edit ops < 1 s, GLB export < 2 s); 8.5 Blender 5.2.x spec refresh routine.

**Critical path**: M0 → 1.1 → 1.3 → 1.5 → (M2 ‖ M3 ‖ 5.1–5.3) → M4 → M6. After M1, up to about 12 WPs can run in parallel.

---

## 18. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| D1 resolved as MIT/Apache | many ops capped at T2/T3, no Suzanne | Clean-room versions behind the same trait, verified against goldens. The tier table shows the gap honestly. |
| Bevel (`bmesh_bevel.cc`, about 8k lines) | M2 slip | Dedicated WP 2.7. Ship seg=1 first. T2 for segments > 1 at tol 1e-4. |
| UV unwrap/pack parity | unequal layouts | Parametrizer port for T1 per island. Packing at T3 permanently. |
| Subsurf ordering, young crates (`opensubdiv-rs`, `manifold-rust`, `draco-core`) | wrong order or abandoned crates | Pinned versions behind frap traits, vendoring allowed, own fallback implementations scoped. |
| EEVEE look parity | low SSIM | Workbench first (more deterministic), LUT-exact colour management, light-unit goldens before the full scene. |
| Studio lights, matcaps, LUT provenance | licensing of captured Blender data | Record provenance per fixture. Ship frap-made assets if the license is unclear. |
| Blender 5.3 API churn | spec drift | `spec/blender-<ver>/` directories, the `blender_api` field in `.frap`, `xtask spec-refresh --check`. |
| Stale-value fidelity (D3/D4) confuses agents | agent errors | `hints[]` with exact fix text. The behaviour itself stays Blender's. |

---

## Appendix A: verification probes run for this proposal (Blender 5.2.4, `-b --factory-startup`)

| # | Probe | Result | Used in |
|---|---|---|---|
| P1 | `o.location=(5,0,0)` then read `matrix_world` | translation `(0,0,0)`: stale | D4, §4.1 |
| P2 | then `bpy.ops.object.select_all(action='DESELECT')` | `(5,0,0)`: an op call evaluates | §4.1 |
| P4 | `view_layer.update()` | fresh | §4.1 |
| P5/P6 | `o.scale=(2,2,2)`: `dimensions` before/after an op | `(2,2,2)` then `(4,4,4)` | D4 |
| Q1 | set parent location after the last op, then `parent_set` | `matrix_parent_inverse` translation `-6`: **evaluation happens before exec** | §6.2 step 3 |
| P7/P8 | `calc_loop_triangles` on two non-planar quads | `(0,1,2),(0,2,3)` both times | §3.1 looptris |
| P9/P10, Q3 | `bpy.data` order after rename and for mixed-case and numeric names | `['AAA','Camera','Cube','Light']`; `['a10','a9','b','B','Cube','Cube.002','Cube.010','Cube.9']`: case-insensitive, non-natural, insertion-stable | §2.3 |
| P11 | `primitive_cube_add(sizee=2)` | `TypeError: Converting py args to operator properties:: keyword "sizee" unrecognized` | §6.2, §9.3 |
| R1 | vertex group added on an object that shares its mesh with another | the twin object sees it: names live on the mesh | §2.4, §3.1 |
| R2 | `modifiers.new('', 'BEVEL')` | name `Bevel`, non-zero `persistent_uid` | `Seg::Modifier` key |
| S1 | `nodes.new("ShaderNodeSeparateRGB")` | `RuntimeError: Error: Node type ShaderNodeSeparateRGB undefined` | §7.6 |
| S2 | `action.fcurves` | `AttributeError: 'Action' object has no attribute 'fcurves'` | §4.4 |
| S3 | `bpy.context.object = None` | `AttributeError: bpy_struct: Context property "object" is read-only` | §5 |
| S4 | `bpy.ops.mesh.foo()` | `AttributeError: Calling operator "bpy.ops.mesh.foo" error, could not be found` | §6.2 step 1 |

The probe scripts are in the session scratchpad and can be moved to `tools/blender/probes/` with WP 0.1.
