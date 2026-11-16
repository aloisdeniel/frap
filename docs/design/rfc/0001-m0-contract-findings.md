# RFC 0001: M0 contract findings (WP 0.2 contract probes)

Status: **accepted** 2026-11-16 by the M1 W0 integrator (WP 1.0) and applied to `docs/ARCHITECTURE.md` v1.2
(see its "Decision log (M1 W0, v1.2)" for the choices this RFC left open: F-TR-6 → blanket impls, F-TR-4 →
direct change of `RnaColl::item`, F-GEN-1 → module-scoped consts, F-DEF-2 → overlay). Disposition of the probe
code: §6.
Author: WP 0.2. Date: 2026-11-16. Blender: 5.2.4 LTS (`cd5dc428cce1`). Rust: 1.97.1, edition 2024.

This RFC lists every contract change the probes found. Each finding gives the evidence, then the exact Rust
signature change for the M1 W0 integrator. Section and type names refer to `docs/ARCHITECTURE.md` v1.1.

## 1. What was probed, and how

`crates/frap-probe/` is a standalone workspace (excluded from the root one, own `Cargo.lock` and `target/`).
Its member crates mirror the real crate boundaries, so orphan rules, layering, cross-crate statics and
`linkme` are exercised for real:

| Probe crate | Stands for | Content |
|---|---|---|
| `rna` | `frap-rna` | §5.1 `id`, §9.1 `def`, §9.2 `value` (+ a reduced `coerce` with Blender wording), §9.3 `ptr`, §9.4 `traits`, §10.1 `op`, §13.4 `error`; `generated/enums.rs` |
| `spec` | `frap-spec-rna` + `frap-spec-ops-{mesh,object}` | **generated only** (`src/generated/**`) |
| `codegen` | `xtask codegen` | dumps → Rust; `--check` and a drift test |
| `mesh` | `frap-mesh` | §6.1 SoA `Mesh`, `MeshVertexRef`/`MeshVertexMut`, `MeshVertices(Mut)`, `IdMaterials` |
| `nodes` | `frap-nodes` | §16.1 `NodeTree`/`Node`/`Socket`/`Link`, first-enabled socket lookup |
| `scene` | `frap-scene` | `Main`/`IdMap`/`ChangeLog`, `Object`, `Material` (embedded tree), `Scene`, `Context`, `HostRead`/`Host`/`Evaluator`, `RNA_HOOKS`, derived `MaterialSlot` |
| `render-scene` | `frap-render-scene` | `RenderScene`, `Film`, `RenderService`, `build` |
| `op` | `frap-op` | `OperatorDef`, `OPERATORS`, `Poll`, `Services`, `OpCtx` |
| `render` | `frap-render` | wgpu 30.0.2 `Gpu`, clear-and-readback, `new_service` |
| `session` | `frap-session` | `Host`+`Services` impl, string-path resolver, transactions, three registered operators, end-to-end tests |

Dumps: `tools/blender/dump_ops.py` and `tools/blender/dump_rna.py` (first versions) were run against the real
Blender and wrote `crates/frap-probe/dumps/` (namespaces `mesh`, `object`: 167 + 250 operators, poll matrix
A–E with and without a VIEW_3D override; structs `Object`, `Mesh`, `MeshVertex`, `Material`, `ShaderNodeTree`,
`NodeSocket`, their base chains, and the prototype extras `MaterialSlot`, `Node`, `NodeSocketColor`,
`ShaderNodeBsdfPrincipled`, `Context`, `Scene`, `ViewLayer`, `ToolSettings`). Two runs produce byte-identical
dumps. `cargo run -p frap-probe-codegen` turns them into 26 files (21,384 lines) with no hand edits.

Acceptance evidence (all green locally on macOS arm64; see §5 for what was not verifiable):

| Acceptance item | Evidence |
|---|---|
| `PropDef`/`StructDef`/`OperatorSpec` generated from the real dumps without hand edits | `spec/src/generated/**`; `codegen --check` clean; `codegen` test `no_drift`; `spec/tests/generated.rs` checks the §9.6 `Object.location` and §10.4 `mesh.primitive_cube_add` values field by field |
| `MeshVertexMut` (SoA proxy) | `mesh/src/rna.rs` unit tests; `session/tests/prototypes.rs::mesh_vertex_soa_proxy_get_set` (get/set through string paths, cache invalidation, Blender error wording) |
| derived `MaterialSlot` | `prototypes.rs::material_slot_derived_get_set` (slot count follows `data.materials`, DATA writes the mesh, OBJECT override grows lazily, repr by index) |
| embedded node-tree path (`Root::Embedded`, Appendix A repr) | `prototypes.rs::embedded_node_tree_path_repr_get_set` reproduces `bpy.data.materials['X'].node_tree.nodes["Principled BSDF"].inputs[0]` and `nodes["Principled BSDF"].inputs[0].default_value`; the write bumps the owner's generation and logs only the owner |
| `Context` | `prototypes.rs::context_members_and_rna_props` |
| `Box<dyn RenderService>` through `OpCtx::services()` | `prototypes.rs::render_service_through_opctx_services`, `render_without_gpu_rolls_back`, `render_service_real_gpu_through_opctx` (Metal) |
| `OpCtx::split_edit` | `prototypes.rs::split_edit_holds_context_and_edit_state` + compile_fail doctest (contract form impossible) |
| wgpu clear-and-readback | `render/tests/gpu.rs`: Rgba32Float 67×33 (padded rows 1072→1280 B), Rgba8Unorm 13×7, a WGSL pipeline draw, the service. Passes on Metal (Apple M1 Max). lavapipe/WARP: `.github/workflows/gpu-probe.yml` |
| contract failures shown, not just claimed | 6 `compile_fail` doctests (`rna/src/lib.rs`, `scene/src/host.rs`, `op/src/lib.rs`); each was also compiled as a normal file to confirm the error is the intended one (E0502, E0277, E0308, reserved keyword) |

Commands: `cd crates/frap-probe && cargo run -p frap-probe-codegen -- --check && cargo test --workspace`
(and `cargo test -p frap-probe-session --no-default-features`). `cargo clippy --workspace --all-targets -- -D warnings` is clean.

## 2. Findings that block a compile (must change)

### F-ED-1 `gen` is a reserved keyword (edition 2024)
`IdHeader.gen` (§5.1) and every `*/src/gen/` module (§2.2.2, §4.2, §22.3 WP 1.1) fail to parse:
`error: expected identifier, found reserved keyword 'gen'`. Doctest in `rna/src/lib.rs`.
```diff
 pub struct IdHeader {
-    pub gen: u64,
+    pub generation: u64,
```
Rename the generated directories `src/gen/` → `src/generated/` everywhere (§2.2 rule 2, §4.2 table, §22.3
WP 1.1 "every `crates/*/src/gen/**`", CI `codegen --check` paths). (`r#gen` works but would leak into every
use site.)

### F-ED-2 `frap-scene/src/main.rs` becomes a binary target
§5.2 places `Main` in `frap-scene/src/main.rs`. Cargo auto-detects `src/main.rs` next to `lib.rs` as a bin
target and fails to build it (`E0601: main function not found`). Rename the file to `src/data.rs`
(module `data`; `pub use data::*`). Probe: `scene/src/data.rs`.

### F-LINK-1 `#![forbid(unsafe_code)]` is incompatible with `linkme`
Every `#[linkme::distributed_slice]` declaration and entry expands to `#[link_section]`, which trips the
`unsafe_code` lint: `error: declaration of a static with link_section`. With `forbid` no local `allow` is
possible. §21.6 requires `forbid` in every crate except three, but `frap-scene` (hooks), `frap-op`
(`OPERATORS`), every `frap-ops-*`, `frap-modifiers`, `frap-io-gltf`, `frap-golden` register through slices.
Change §21.6 to:
> `#![deny(unsafe_code)]` in every crate; the only allowed exception is `#[allow(unsafe_code)]` on a
> `linkme` slice declaration or entry (generated by `#[operator]`, `#[rna_func]`, `#[rna_hook]`, `#[modifier]`,
> `#[bmesh_op]`, so hand-written code never needs it). `frap-render`, `frap-codec-mesh`, `frap-imbuf` keep
> `allow` with a `// SAFETY:` comment per block.

The proc-macros must emit the `#[allow(unsafe_code)]` themselves.

### F-OP-5 `OpCtx::split_edit` cannot be built on the §9.5 `Host`
`host.ctx()` borrows `*host` shared and `host.edit_mut()` borrows it mutably → `E0502` (doctest in
`op/src/lib.rs`). Add a split method to the trait:
```diff
 pub trait Host: HostRead {
     fn edit_mut(&mut self) -> &mut EditState;
+    /// Context (main, window, prefs) and the edit state at once: disjoint fields of State.
+    fn split_edit(&mut self) -> (Context<'_>, &mut EditState);
```
`OpCtx::split_edit` then forwards to it (probe: `op/src/lib.rs`, `session/src/lib.rs`).

### F-OP-6 `OpCtx::split_eval` returns something the evaluator cannot use
The contract `split_eval(&mut self) -> (Context<'_>, &mut dyn Evaluator)` compiles, but
`Evaluator::object(&mut self, host: &dyn HostRead, …)` needs a `HostRead`, and a `Context` cannot be one (no
`EditState`, so no `edit()`/`mesh_view()`): `E0277 Context: HostRead is not satisfied` (doctest).
```diff
-    pub fn split_eval(&mut self) -> (Context<'_>, &mut dyn Evaluator);
+    pub fn split_eval(&mut self) -> (&dyn HostRead, &mut dyn Evaluator);   // ctx via HostRead::ctx()
```

### F-HOOK-1 The get-hook signature cannot serve `=> hook` / `hook_coll` reads
```rust
pub type RnaGetHook = fn(&mut dyn Host, this: &RnaPtr, prop: &'static PropDef) -> Result<Value, RnaError>;
```
1. Reads take `&dyn HostRead` (no transaction, §12; `frap-golden` snapshots through RNA). A read cannot call a
   hook that needs `&mut dyn Host` (`E0308`, doctest in `scene/src/host.rs`).
2. `Object.material_slots => hook_coll` (§9.6) must hand back a collection proxy that borrows the scene so
   that `material_slots[0].material` keeps resolving. An owned `Value` cannot do that.
```diff
-pub type RnaGetHook = fn(&mut dyn Host, this: &RnaPtr, prop: &'static PropDef) -> Result<Value, RnaError>;
+pub type RnaGetHook = for<'h> fn(&'h dyn HostRead, this: &RnaPtr, prop: &'static PropDef)
+                                  -> Result<Field<'h>, RnaError>;
```
Probe: `scene/src/slots.rs` (get hook returns `Field::Coll(CollRef::Owned(MaterialSlots { main, ob }))`).
`RnaSetHook` and `RnaFuncImpl` are fine as specified. Define the registration struct the contract only names:
```rust
pub struct RnaHookReg { pub srna: &'static str, pub prop: &'static str,
                        pub get: Option<RnaGetHook>, pub set: Option<RnaSetHook> }
```
Lookup walks the `base` chain (a hook on `ID.name` serves every ID type).

### F-TR-3 A binding cannot say "this getter is a hook"
`SetOutcome::NeedsHook` exists for setters; there is no getter equivalent, so the resolver cannot know when to
call `RNA_HOOKS` for `material_slots`, `matrix_world`, `dimensions`, `users`, … Add (additive, see F-TR-1):
```diff
 pub enum Field<'a> { Value(Value), Struct(StructRef<'a>), Id(Option<IdAny>), Coll(CollRef<'a>),
+    NeedsHook,
```
`rna_struct!` emits `Ok(Field::NeedsHook)` for `=> hook` / `hook_coll` bindings.

### F-TR-1 / F-TR-2 Embedded IDs and canonical pointers have no `Field` variant
`Material.node_tree` is an ID pointer (`is_id`) with no `IdKey`, so it is neither `Field::Id` nor a plain
`Field::Struct` (the resolver must re-root at `Root::Embedded { owner, slot }`). Context members that point
*into* an ID must re-root at the canonical path, because Blender's repr is canonical (verified:
`bpy.context.tool_settings` → `bpy.data.scenes['Scene'].tool_settings`,
`bpy.context.view_layer` → `bpy.data.scenes['Scene'].view_layers["ViewLayer"]`).
```diff
 pub enum Field<'a> {
     Value(Value), Struct(StructRef<'a>), Id(Option<IdAny>), Coll(CollRef<'a>),
+    Embedded(EmbeddedSlot, StructRef<'a>),   // re-root at Root::Embedded { owner: <current ID>, slot }
+    Ptr(RnaPtr),                             // re-root at a canonical pointer (resolved from its root)
+    NeedsHook,                               // F-TR-3
 }
```
Mark `Field` `#[non_exhaustive]`.

### F-TR-4 Collection items cannot be IDs or `None`
`RnaColl::item -> StructRef` cannot express an ID item (`bpy.data.*`, `context.selected_objects`,
`collection.objects`, `mesh.materials`) nor a `None` item (`mesh.materials.append(None)` is legal, verified)
(`E0308`, doctest in `rna/src/lib.rs`). ID-item collections also cannot answer `key_of`/`find` from a low
crate (`frap-mesh` cannot see material names, L4 < L6); the resolver must search them through `IdNames`
(verified: `mesh.materials['Material']` works in Blender).
```diff
+pub enum Item<'a> { Struct(StructRef<'a>), Id(Option<IdAny>) }
 pub trait RnaColl {
-    fn item(&self, i: usize) -> Result<StructRef<'_>, RnaError>;
+    fn item(&self, i: usize) -> Result<Item<'_>, RnaError>;
```
(The probe kept `item` and added `item_any` with a default to stay additive; for the frozen contract the
direct change is cleaner.)

### F-VAL-4 `Value` has no ID variant; the §10.4 example does not compile
`cx.result("object", ob)` needs `IdRef<ObjectId>: Into<Value>` (`E0277`, doctest). `Value::Ptr(RnaPtr)` needs a
`&'static StructDef`, which `frap-rna` (L1) cannot name (the statics are generated in L2), and the impl would be
an orphan in any other crate. JSON already distinguishes `{"$ref": …}` (§13.3).
```diff
 pub enum Value {
     None, Bool(bool), Int(i64), Float(f32), Double(f64), Str(SmolStr),
     Enum(SmolStr), EnumSet(BTreeSet<SmolStr>),
     BoolArray(SmallVec<[bool; 4]>), IntArray(SmallVec<[i64; 4]>),
     FloatArray { data: SmallVec<[f32; 16]>, shape: ArrayShape, tag: MathTag },
-    Ptr(RnaPtr), Coll(RnaPtr), Tuple(Vec<Value>),
+    Id(Option<IdAny>),                    // ID pointer; impl<K: IdKind> From<IdRef<K>> for Value in frap-rna
+    Ptr(Box<RnaPtr>), Coll(Box<RnaPtr>),  // F-VAL-1
+    Tuple(Vec<Value>),
+    Structs(Vec<IndexMap<SmolStr, Value>>),  // F-VAL-3
 }
```

### F-LAYER-1 `Report` is in the wrong crate
§3.4 puts `Report` in `frap-op` (L9), but `Host::report(r: Report)` and `EvaluatedObject.warnings: Vec<Report>`
(frap-scene, L6), `ModEvalCx::report` (L7) and `LoweredMaterial.diagnostics: Vec<Report>` (L7) name it. Move
`Report { level: ReportType, message: String }` and `ReportType { Info, Warning, Error, Operator, Property }` to
`frap-rna/src/error.rs` (probe: `rna/src/error.rs`), re-exported by `frap-op`.

### F-REN-2 `GpuOptions` must not live in `frap-render`
`SessionServices.render_factory: fn(&GpuOptions) -> …` (§12) must compile when the `gpu` feature (and so
`frap-render`, the only wgpu crate) is off (§3.6). Move `GpuOptions { backend: Option<String>, force_fallback: bool }`
(+ `from_env()` for `FRAP_WGPU_BACKEND`, `FRAP_FORCE_FALLBACK`) to `frap-render-scene::types`; `frap-render`
re-exports it. Verified: `cargo test -p frap-probe-session --no-default-features` builds without wgpu.

### F-OP-7 Render/export have no contract path to a State clone
§7.3/§19.4 render and export "on a COW clone of `State` with animation applied", but an operator only sees
`OpCtx` → `&mut dyn Host`; nothing clones State. Add:
```rust
// frap-scene/src/host.rs
pub struct Snapshot { pub host: Box<dyn HostRead + Send>, pub eval: Box<dyn Evaluator + Send> }
pub trait Host: HostRead {
    /// COW clone of State with pending animation applied at `frame` (scene frame if None);
    /// the evaluator is fresh (its cache must not mix clone and original generations).
    fn snapshot_at(&mut self, frame: Option<i32>) -> Result<Snapshot, RnaError>;
}
// frap-op
impl OpCtx<'_> { pub fn snapshot_at(&mut self, frame: Option<i32>) -> Result<Snapshot, OpError>; }
```
Probe: `frap.probe_render` builds the `RenderScene` from `snapshot_at(None)` and renders through
`cx.services().render()?`.

### F-OP-9 `OpCtx` cannot create datablocks other than objects
`object_mut`/`mesh_mut`/`add_object` cover two ID types; `material.new` (WP 1.20), `camera_add`/`light_add`
(data), `collection.*`, `world.new`, import need inserts into other `IdMap`s, and helpers like
`add_primitive_or_merge` need a mesh insert. Add `pub fn main_mut(&mut self) -> (&mut Main, &mut ChangeLog)`
(probe: `op/src/lib.rs`; used by `mesh.primitive_cube_add`).

### F-OP-8 `OpCtx` has no constructor and no way out
Fields are private (good) but `frap-session` must build one and collect `reports`/`result`:
`pub fn new(host: &'h mut dyn Host, services: &'h mut dyn Services, op: &'static OperatorSpec) -> Self` and
`pub fn finish(self) -> OpOutput { reports, result }`.

### F-SES-1 `Session` cannot itself be the `Host`
`OpCtx::new` needs `&mut dyn Host` and `&mut dyn Services` simultaneously. If `Session` implements `Host` and
also owns `services`, the two borrows overlap. The `Host` must be a field disjoint from `services`
(probe: `Session { host: HostImpl { state, eval, log, reports }, services: SessionServices }`). Change the §9.5
comment "implemented by frap-session::Session" to "by `frap_session::HostImpl`, a field of `Session`".

## 3. Findings that change shapes or add members (should change)

### F-VAL-1 `Value` is 200 bytes
`size_of::<Value>() == 200`, `RnaPtr == 192` (`SmallVec<[Seg; 6]>`, `Seg` holds a `SmolStr`), `Field == 200`,
`PropDef == 136` (`rna/src/traits.rs::tests::sizes`). Every `Vec<Value>`/`OpArgs`/`&[Value]` pays this.
Box the pointer variants (`Ptr(Box<RnaPtr>)`, `Coll(Box<RnaPtr>)`, 8 B each). `Value` is then bounded by
`FloatArray` (`SmallVec<[f32; 16]>` alone is 80 B, so ≈ 96 B); an inline capacity of 4 (vectors, colours,
quaternions; matrices spill) would bring it near 40 B.

### F-TR-7 Bulk access needs a dense buffer
`foreach_set(&mut self, prop, flat: &[Value])` costs 200 B per scalar: a 1M-vertex `foreach_set("co", …)` is
600 MB of `Value`s. `foreach_get -> Value` loses the `[N, 3]` shape (`ArrayShape` has no "N items of 3").
```rust
pub enum Bulk { F32(Vec<f32>), I32(Vec<i32>), Bool(Vec<bool>) }   // + shape on the side: (n_items, item_len)
fn foreach_get(&self, prop: &'static PropDef, range: Range<usize>) -> Result<Bulk, RnaError>;
fn foreach_set(&mut self, prop: &'static PropDef, data: &Bulk) -> Result<(), RnaError>;
```
`coerce` produces `Bulk` from an `Input::List` once; the §13.3 `$array` encoder reads `Bulk` directly.
Probe: `mesh/src/rna.rs` implements both forms. Verified 5.2.4 wording for a wrong length:
`RuntimeError: internal error setting the array` (plus an ERROR report `Array length mismatch (got 5, expected more)`),
not a ValueError.

### F-TR-5 The canonical item segment is per collection
`repr`/`path_from_id` use the name for some collections and the index for others, regardless of how the item
was looked up (verified): `nodes["Principled BSDF"]`, `modifiers["Bevel"]`, `attributes["position"]` by name;
`inputs[0]` (looked up as `inputs['Base Color']`) and `material_slots[0]` (looked up as
`material_slots['Material']`) by index; `vertices[3]` (nameless). `StructDef.name_property` does not decide it
(`NodeSocket` and `MaterialSlot` both have one). Add to `RnaColl`:
```rust
/// Canonical repr/path segment of item i. Default: Key(name) if key_of(i) is Some, else Index(i).
fn item_seg(&self, i: usize) -> Seg { … }
```
The resolver must store canonical segments in `RnaPtr.path` (never the user's `Key` for sockets/slots).

### F-TR-6 Boxing a reborrow needs blanket impls
`RnaMutGuard<'a> = Box<dyn RnaMut + 'a>` forces in-memory structs (nodes, sockets, settings) to be boxed as
`Box::new(&mut node)`, which requires `impl<T: RnaMut + ?Sized> RnaMut for &mut T` (and `Rna for &mut T`) in
`frap-rna`. Add both blanket impls to the contract (probe: `rna/src/traits.rs`). Alternative with no allocation
per path segment, symmetric with `StructRef`:
`pub enum RnaMutGuard<'a> { Borrowed(&'a mut dyn RnaMut), Owned(Box<dyn RnaMut + 'a>) }` + `DerefMut`.
Either is fine; pick one before the freeze.

### F-TR-8 `RnaColl` needs `is_empty`
`cargo clippy -D warnings` (§20.5) fails on the contract trait itself (`len_without_is_empty`). Add
`fn is_empty(&self) -> bool { self.len() == 0 }`.

### F-ID-1..4 Identity helpers the resolver and hooks need
```rust
impl IdCode { pub const fn collection(self) -> &'static str; }          // repr of a dynamic IdAny (F-ID-1)
impl<K: IdKind> IdRef<K> {
    pub fn from_key(k: IdKey) -> Self; pub fn key(self) -> IdKey;
    pub fn from_any(a: IdAny) -> Option<Self>; pub fn any(self) -> IdAny;  // checked by code (F-ID-2)
}
impl IdKey { pub fn next() -> IdKey; }                                  // process-global (F-ID-3)
impl EmbeddedSlot { pub const fn prop_identifier(self) -> &'static str; } // "node_tree" | "collection" (F-ID-4)
```
`IdRef<K>` needs manual `Clone/Copy/Eq/Hash/Ord/Debug` impls (as the contract comment implies; derives would
add `K:` bounds).

### F-PTR-1 `IdNames` is undefined
`RnaPtr::repr(&self, names: &dyn IdNames)` names a trait that does not exist. Define in `frap-rna/src/ptr.rs`:
```rust
pub trait IdNames { fn id_name(&self, id: IdAny) -> Option<(SmolStr, Option<SmolStr /*library path*/>)>; }
```
implemented by `Main`. Linked IDs repr as `bpy.data.node_groups['Name', '<lib path>']` (exact library string
pinned by WP 1.10, see §5.2 note). Quoting rules (verified): single-quoted Python repr at the root with `\'`
escaped (`bpy.data.objects['a\'b"c']`), double-quoted keys below.

### F-MAIN-1/2 `Main` and `IdMap` accessors
The resolver and hooks reach IDs through `IdAny`:
```rust
impl Main {
    pub fn id(&self, a: IdAny) -> Option<&dyn Rna>;
    pub fn id_mut(&mut self, a: IdAny, log: &mut ChangeLog) -> Option<&mut dyn RnaMut>;  // via get_mut
    pub fn header(&self, a: IdAny) -> Option<&IdHeader>;
    pub fn collection(&self, attr: &str) -> Option<(Vec<IdAny>, &'static StructDef)>;    // bpy.data.<attr>
}
impl<T: IdType> IdMap<T> { pub fn get_arc(&self, r: IdRef<T::Kind>) -> Option<Arc<T>>; }  // mesh_view in object mode
```

### F-DEF-1 `PropFlags` misses flags the dump has and frap needs
Add `ARGUMENT_OPTIONAL` (function params), `DEPRECATED`, `PATH_SUPPORTS_BLEND_RELATIVE` (`//` paths:
`render.filepath`), `SKIP_PRESET`, `CONTEXT_MEMBER` (F-DEF-5). `bitflags` 2 with `.union()` (const) works in
generated statics.

### F-DEF-2 `FloatStorage` cannot come from the dump
bpy exposes no storage width. Either drop `storage` (all RNA floats are f32 except a handful of double props) or
fill it from an overlay list. The probe emits `F32` everywhere.

### F-DEF-3 Deprecation metadata
5.2 exposes `is_deprecated`, `deprecated_note`, `deprecated_version`, `deprecated_removal_version`
(`Material.use_nodes`: note "Unused but kept for compatibility reasons…", 5.0.0 → 6.0.0). frap must emit
Blender's DeprecationWarning report. Add `pub deprecated: Option<&'static Deprecation>` to `PropDef` with
`Deprecation { note: &'static str, version: [u16; 3], removal_version: [u16; 3] }`.

### F-DEF-4 Function metadata that is not dumpable
RNA functions carry no keyword-only marker (`FuncDef.keyword_only_from` only comes from
`python_only.toml`, e.g. `keyframe_insert`) and no `USE_CONTEXT` flag; bpy exposes `use_self` and
`use_self_type` instead. Replace `FuncFlags::{USE_SELF_ID, USE_CONTEXT}` by `NO_SELF`, `USE_SELF_TYPE` (dumped)
and document `keyword_only_from` as overlay-only.

### F-DEF-5 Context members are not RNA properties
`bpy.types.Context.bl_rna.properties` has 20 entries (`scene`, `view_layer`, `mode`, `tool_settings`, …) but
**not** `object`, `active_object`, `selected_objects`, `edit_object`, `objects_in_mode`, … (they come from C
context callbacks). The resolver would answer `AttributeError` for `bpy.context.object`. The probe's
`dump_rna.py` records `context_members` from `bpy.context.copy()` in four states inside a VIEW_3D override (51
members) and codegen synthesises `PropDef`s for them with `READONLY | CONTEXT_MEMBER`. 29 of the 51 were never
non-`None`, so their types need `spec/overlay/context_members.toml`. Their write error differs from RNA
props (verified): `bpy_struct: Context property "object" is read-only` vs
`bpy_struct: attribute "scene" from "Context" is read-only`.

### F-DEF-6 Dynamic enums: the dump cannot detect them
In `-b`, 22 operator enums have **empty** item lists (`object.constraint_add.type`,
`object.vertex_group_set_active.group`, `object.make_links_scene.scene`, …) and two have a default not among the
items (`object.datalayout_transfer.data_type = ""`, `object.empty_image_add.sort_method = ""`).
`object.mode_set.mode` returns the full static list, so comparing contexts does not flag it (only
`object.empty_image_add.sort_method` differed between contexts A–E). `dynamic_enums.toml` is therefore required
for correctness, not only completeness; `coerce` must consult it before rejecting an enum. Typed props structs
use `SmolStr` for these (the probe does).

### F-DEF-7 RNA defaults are not fresh values
Confirmed and quantified for `Object`: `matrix_world/local/basis` RNA default is all zeros (fresh: identity),
`track_axis` (fresh `POS_Y`), `up_axis`; `MeshVertex.select` (fresh `True`); `Mesh.normals_domain` (fresh
`POINT`). `dump_rna.py` writes `fresh.differs_from_default` per struct with the creation recipe. Read-only props
(`id_type`, `is_editable`) also differ; generated `Default` impls should ignore read-only props.

### F-GEN-1 `<STRUCT>_<PROP>` const names collide in the full API
Scanning all 4,011 structs / 10,158 own props of Blender 5.2.4: `NodesModifier.bake_directory` and
`NodesModifierBake.directory` both map to `NODES_MODIFIER_BAKE_DIRECTORY` (the `pub use <m>::*` re-exports
then clash). Generate module-scoped consts (`frap_spec_rna::object::LOCATION`, re-exported per module only)
or a double underscore (`OBJECT__LOCATION`). The codegen must fail on any collision (the probe reports them).

### F-GEN-2 Typed props structs: naming facts
In mesh+object: 37 operators have a prop named `type` (`r#type` fields), enum identifiers `'2D'`/`'3D'`
(`object.grease_pencil_add.stroke_depth_order`) need a prefix (`V2d`), `'x'/'y'/'z'` (`object.paste_transform`)
are lower case; no variant collisions. One operator collection prop (`object.volume_import.files`,
`OperatorFileListElement`) has no `Value` representation (F-VAL-3); `import_scene.gltf` has the same `files`
prop.

### F-VAL-3 Operator collection props
`files=[{"name": "a.glb"}]` cannot be coerced: `Value` has no list-of-structs variant. Add
`Value::Structs(Vec<IndexMap<SmolStr, Value>>)` (see F-VAL-4 diff) and `Prop<Vec<…>>` in typed structs.

### F-VAL-2 `CoerceCx` is undefined
Blender's message prefix depends on the assignment site (verified):
`bpy_struct: item.attr = val: Object.name expected a string type, not int` vs
`Converting py args to operator properties: MESH_OT_primitive_cube_add.size expected a float type, not str`.
```rust
pub enum CoerceCx { RnaSet { owner: &'static str /*struct id*/ }, OpArgs { owner: &'static str /*bl_idname*/ } }
```
More verified wording for the `coerce` table: bool `expected True/False or 0/1, not int`; int
`expected an int type, not float`; items `expected sequence items of type float, not str`; a str is a sequence
(`location = 'x'` → ValueError `… should contain 3 items, not 1`); scalar to array → TypeError with an empty
message tail (`bpy_struct: item.attr = val: `); pointer `MaterialSlot.material expected a Material type, not Object`;
`pass_index = -5` clamps silently to 0.

### F-OP-1..4 Small `frap-rna/src/op.rs` gaps
- `OpArgs.is_set: BitSet`: no bit-set crate in §3.5 (glTF export has 110 props). Define
  `pub struct BitSet(SmallVec<[u64; 2]>)` in `frap-rna`.
- Generated `from_args` needs `trait FromValue { fn from_value(v: &Value) -> Self; }` (infallible after
  `coerce`) with impls for `bool, i32, f32, SmolStr, [f32; N], [[f32; C]; R], [bool; N], [i32; N],
  BTreeSet<SmolStr>` and generated enums, plus `OpArgs::prop::<T>(index, identifier) -> Prop<T>`.
- `OperatorSpec.poll_matrix` is meaningless for hand-written `frap.*` specs: `Option<PollMatrix>`.
  Define `PollMatrix { plain: [PollCell; 5], view3d: [PollCell; 5] }`, `PollCell { Y, N, Raised }`.
- §10.4 uses `Prop::map`; add `impl<T> Prop<T> { pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Prop<U> }`.

### F-NODE-1 Socket and link names cannot be `&'static str`
Group-node sockets take their names and identifiers from the group interface (user data), and `.frap` load
cannot produce `&'static str` without leaking.
```diff
-pub struct Socket { pub name: &'static str, pub identifier: &'static str, …
+pub struct Socket { pub name: SmolStr, pub identifier: SmolStr, …
-pub struct Link { pub from_node: u32, pub from_socket: &'static str, pub to_node: u32, pub to_socket: &'static str }
+pub struct Link { pub from_node: u32, pub from_socket: SmolStr, pub to_node: u32, pub to_socket: SmolStr }
```

### F-DEP-1 `bitflags` is not a workspace dependency
`PropFlags`, `FuncFlags`, `OpOptions`, `BMesh`'s `ElemFlag` (§6.2 uses `bitflags!` literally) need it. Add
`bitflags = "2.13"` to §3.5.

### F-REN-1 / F-EVAL-1 Undefined names
`RenderRequest` (§19.1 `build(…, &RenderRequest)`), `EvalError` (§7.3), `PathResolver`, `ImageCache`,
`BoneRef` are named but not defined. The probe's minimal versions are in `render-scene/src/lib.rs`,
`scene/src/host.rs`, `op/src/lib.rs`.

### F-REN-3 wgpu 30.0.2 API notes for WP 1.7
`InstanceDescriptor::new_without_display_handle()` (struct has a `display` field);
`RequestAdapterOptions` gained `apply_limit_buckets` (use `..Default::default()`); `BufferSlice::get_mapped_range`
returns `Result`; `RenderPassColorAttachment.depth_slice`, `RenderPassDescriptor.multiview_mask`,
`RenderPipelineDescriptor.multiview_mask`; `Device::poll(PollType::Wait { submission_index, timeout })`;
FXC via `desc.backend_options.dx12.shader_compiler = Dx12Compiler::Fxc`. `Rgba32Float` is a valid render
attachment with `blend: None` on Metal. First pipeline + draw + readback: 4 ms on Metal (M1 Max).

## 4. Things that held (no change)

- `PropDef`/`StructDef`/`OperatorSpec`/`EnumItem` as `static` data, including `pub const OBJECT_LOCATION:
  &PropDef = &OBJECT_PROPS[16];` (a const referring to a static; pointer-identical to `find_prop`'s result).
  Clean build of the generated crate alone (19 structs, 417 operators all with typed props structs, 21k lines,
  1.8 MB of source): ~3.0 s debug, ~3.1 s release on an M1 Max; release rlib 9.9 MB. This is the first data
  point for the §4.2 build-time budgets.
- `StructRef::Owned` proxies (`MeshVertexRef { mesh: &Mesh, i }`), `RnaMutGuard` proxies
  (`MeshVertexMut { mesh: &mut Mesh, i }`) and a recursive mutable walk
  (`coll_mut → item_mut → set`) compile and run; caches are cleared because the proxy writes through
  `Mesh`'s `&mut` methods; `Arc::make_mut` on a snapshot-shared `Mesh` yields a cold cache.
- `Rna: Send + Sync`, `Context<'a>: Rna`, `Mesh: Send + Sync`, `Gpu`/`Renderer: Send`.
- Embedded IDs mutate through the owner (`main.id_mut(owner) → struct_mut("node_tree") → …`): owner
  generation bumps, `ChangeLog.modified == [owner]`.
- Derived `MaterialSlot` works with hooks once F-HOOK-1 is applied; writes to a DATA-linked slot land in the
  **mesh** (verified Blender behaviour), so the set hook needs `&mut dyn Host` (as specified).
- `OperatorDef` is const-constructible in a `linkme` static from a generated `&'static OperatorSpec`;
  `fn(&mut OpCtx, &OpArgs) -> OpResult` coerces from a plain `fn`. `OpCtx::run_op` can build a nested
  `OpCtx` reborrowing host and services.
- `Services::render(&mut self) -> Result<&mut dyn RenderService, OpError>` works with a lazily created
  `Option<Box<dyn RenderService>>` (`&mut (dyn RenderService + 'static)` shortens to `+ '_`).
- Trait upcasting `&dyn Host → &dyn HostRead` (stable since 1.86) lets one resolver serve reads and writes.
- The dumps are deterministic (two runs byte-identical); the poll matrix had 0 raising polls; 28 of 417
  operators change result with a VIEW_3D override (mostly `mesh.polybuild_*`, knife, loop/ring select), which
  confirms the virtual 3D View requirement (§8).

## 5. Not verifiable locally

- **lavapipe (Linux) and WARP (Windows)** runs: only configured in `.github/workflows/gpu-probe.yml` (Mesa
  `mesa-vulkan-drivers` from apt with `VK_DRIVER_FILES` pinned to the lavapipe ICD; WARP 1.0.20 from NuGet copied
  next to the test binaries, as wgpu's own CI does; `FRAP_EXPECT_ADAPTER` asserts the adapter name). Locally,
  `cargo check --workspace --all-targets` passes for `x86_64-pc-windows-msvc` and `x86_64-unknown-linux-gnu`
  (type-checks the wgpu DX12/Vulkan backends and `linkme` on both), but nothing was linked or run there.
- Runner image names (`macos-15`, `ubuntu-24.04`, `windows-2025`) and whether Ubuntu 24.04's apt lavapipe is
  new enough for wgpu 30 (fallback: wgpu's `gfx-rs/ci-build` Mesa archive) are unverified until the workflow runs.
- `linkme` entry retention under release/LTO was not tested (that is the §10.5 registry test, WP 1.0).

## Appendix: Blender 5.2.4 facts verified by this WP

| Probe | Result |
|---|---|
| `repr(inputs['Base Color'])`, `.path_from_id('default_value')` | `bpy.data.materials['X'].node_tree.nodes["Principled BSDF"].inputs[0]`; `nodes["Principled BSDF"].inputs[0].default_value` (socket srna `NodeSocketColor`) |
| `repr(material_slots['Material'])` | `bpy.data.objects['Cube'].material_slots[0]` |
| `repr(mesh.vertices[3])`, `repr(mesh.attributes['position'])` | `bpy.data.meshes['Cube'].vertices[3]`; `bpy.data.meshes['Cube'].attributes["position"]` |
| `repr(C.tool_settings)`, `repr(C.view_layer)`, `repr(C.layer_collection)` | `bpy.data.scenes['Scene'].tool_settings`; `bpy.data.scenes['Scene'].view_layers["ViewLayer"]`; `bpy.data.scenes['Scene']...LayerCollection` (no path: repr is not always a path) |
| `repr(node_tree.links[0])` | `bpy.data.materials['X'].node_tree` (NodeLink has no path) |
| `repr(objects["a'b\"c"])` | `bpy.data.objects['a\'b"c']` |
| `slot.material = M` with `link='DATA'` | writes `mesh.materials[i]`; with `link='OBJECT'` writes the object override (reads `None` until set) |
| `slot.name` with no material | `''` |
| `mesh.materials.append(None)`; `mesh.materials[i]` | `None` item |
| `vertex.hide = True` then `False` | `.hide_vert` created and kept |
| `mesh.vertices['x']` | `TypeError: bpy_prop_collection[key]: Mesh.vertices does not support string lookups` |
| `vertices.foreach_set('co', [0.0]*5)` | `RuntimeError: internal error setting the array` + report `Array length mismatch (got 5, expected more)` |
| `C.object = None` / `C.scene = None` / `C.mode = 'x'` | `bpy_struct: Context property "object" is read-only` / `bpy_struct: attribute "scene" from "Context" is read-only` / `… attribute "mode" from "Context" is read-only` |
| `bpy.types.Context.bl_rna.properties` | 20 props; `object`, `active_object`, `selected_objects`, … are not among them |
| `material.node_tree = None` | `AttributeError: bpy_struct: attribute "node_tree" from "Material" is read-only` |
| `socket.default_value = (1,2,3)` on a colour socket | `ValueError: … should contain 4 items, not 3` |
| Metallic socket `default_value = 2.0` | stored as 2.0 (no clamp) |
| `object.mode_set.get_rna_type().properties['mode'].enum_items` | full static list (14 items) even with no active object |
| `ID.name.length_max`, `Node.name.length_max` | 256, 64 (= 255 / 63 bytes + NUL) |
| `Material.use_nodes` | `is_deprecated`, note "Unused but kept for compatibility reasons. Setting the property has no effect, and getting it always returns True.", 5.0.0 → 6.0.0 |
| All `bpy.types` structs | 4,011 structs, 10,158 own props; one `<STRUCT>_<PROP>` collision (F-GEN-1) |

## 6. Disposition of the probe (M1 W0)

`crates/frap-probe` was deleted from the workspace in M1 W0. Nothing was lost:

| Probe artefact | Moved to | Owner (M1) |
|---|---|---|
| `dumps/ops/{mesh,object,_meta}.json`, `dumps/rna/*.json` (Blender 5.2.4, byte-stable) | `spec/blender-5.2/ops/`, `spec/blender-5.2/rna/` (seed dumps; re-run by `tools/blender/dump_{ops,rna}.py`) | WP 1.1 |
| `render/` (`Gpu::new` backend policy, padded readback, WGSL draw) and `render/tests/gpu.rs` | `crates/frap-render/src/gpu.rs` (contract `Gpu::new`), `crates/frap-render/src/readback.rs`, `crates/frap-render/tests/gpu.rs`; CI: `.github/workflows/gpu.yml` (replaces `gpu-probe.yml`) | WP 1.7 |
| amended contract types (`rna`, `scene/host.rs`, `op`, `render-scene`, `nodes` types) | frozen in the real crates (`frap-rna`, `frap-scene/src/{host,context,data,eval}.rs`, `frap-op/src/{def,ctx,services,modes}.rs`, `frap-render-scene/src/types.rs`, `frap-nodes/src/tree.rs`) | WPs 1.2, 1.10, 1.11, 1.12, 1.15 |
| `codegen/` (dumps → `StructDef`/`PropDef`/`OperatorSpec` + typed props structs, `--check`, drift test) | archived as source in `docs/design/rfc/0001-probe/codegen/`; starting point for `xtask/src/codegen/` | WP 1.1 |
| prototypes `mesh/src/rna.rs` (`MeshVertexRef`/`MeshVertexMut`, SoA `foreach_*`), `scene/src/slots.rs` (derived `MaterialSlot` + hooks), `scene/src/context.rs` (Context as `Rna`), `nodes/src/lib.rs` (first-enabled socket lookup), `session/src/resolve.rs` (re-rooting resolver), `session/tests/prototypes.rs` | archived as source in `docs/design/rfc/0001-probe/` (Cargo manifests renamed `Cargo.toml.txt` so nothing builds them) | WP 1.27, 1.10, 1.11, 1.14 |
| generated code (`spec/src/generated/**`, `rna/src/generated/**`, 21k lines) | not kept: regenerable from the dumps with the archived codegen | — |

