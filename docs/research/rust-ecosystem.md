# frap: Rust crate ecosystem research (November 2026)

Status: research input for architecture. No code decisions are binding until the architecture doc adopts them.
Data gathered 2026-11-16 from the crates.io API (`/api/v1/crates/<name>`), crate source tarballs (build scripts and feature lists read, never executed), the GitHub API, and a local **Blender 5.2.4 LTS** run (`-b --factory-startup`) for the Blender-side facts that drive crate choice.

Legend: **Adopt** = use as a dependency. **Spike** = promising but young; evaluate behind a feature flag or trait before committing. **Reference** = use as a test oracle or porting reference, not a runtime dependency. **Avoid** = do not use.

---

## 0. Findings that change the architecture

1. **Write our own glTF JSON model.** `gltf`/`gltf-json` are stuck at **1.4.1 (May 2024)**. They lack typed support for clearcoat, sheen, iridescence, anisotropy, dispersion, Draco, meshopt, `KHR_animation_pointer`, and `EXT_mesh_gpu_instancing`. Their validation also rejects any `extensionsRequired` entry they do not know (`root.rs`: `Error::Unsupported`). frap must read *and write* every extension losslessly, so the gltf crates are not a sufficient base.
2. **Blender 5.2's glTF add-on (v5.2.41) references these extensions**, so frap's coverage target is wider than the brief listed: `KHR_animation_pointer` (heavily used), `KHR_meshopt_compression` (the Khronos successor to `EXT_meshopt_compression`, both present), `EXT_mesh_gpu_instancing`, `KHR_materials_pbrSpecularGlossiness` (import), `KHR_materials_unlit`, and `KHR_materials_variants`.
3. **Blender cannot export KTX2/Basis.** `export_image_format` is `['AUTO','JPEG','WEBP','NONE']`. Blender's meshopt path runs an **external `gltfpack` binary** (`export_use_gltfpack`, `export_gltfpack_*` properties). frap can do both in-process. KTX2 output therefore needs a frap-namespaced option, because Blender has no parameter name for it.
4. **Boolean solvers in 5.2 are `FLOAT`, `EXACT`, `MANIFOLD`.** `MANIFOLD` is elalish/manifold, and a **pure-Rust port of Manifold (`manifold-rust`)** exists. That gives Blender-parity booleans with no C++ toolchain.
5. **The Subdivision Surface modifier is OpenSubdiv underneath** (`uv_smooth`, `boundary_smooth`, `use_creases`, `use_limit_surface`, `quality`). A **pure-Rust OpenSubdiv port (`opensubdiv-rs`)** appeared in Aug 2026. It is promising but very young.
6. **`uv.unwrap` methods are `ANGLE_BASED` (ABF++), `CONFORMAL` (LSCM) and `MINIMUM_STRETCH` (SLIM).** No crate implements these the way Blender does. Plan to write them on top of `faer` sparse solvers.
7. **Avoid all C++ deps that use cmake.** `manifold3d` → `manifold-csg-sys` **runs `git clone` of upstream inside `build.rs`, then cmake and TBB**. The `draco-rs` FFI and `opensubdiv-petite` also need C++ toolchains. Any crate that uses `bindgen` at build time (`xatlas-rs-v2`, `basisu_c_sys`) needs **libclang**, which is painful on Windows. Plain `cc`-compiled C/C++ is fine on all three targets: `meshopt`, `libwebp-sys`, and mlua's vendored Lua.
8. **Supply-chain caution.** 2026 brought many new "pure-Rust port of X" crates. Some have high version churn and few users: `draco-core` (6 stars), `opensubdiv-rs` (4 stars), `meshoptimizer-rs` (2 stars), `manifold-rust` (36 stars), and others. Several are probably machine-written. Pin exact versions, read the code, keep each one behind a frap-owned trait, and cover it with golden tests against Blender.
9. **Matching Blender's numbers needs Blender's algorithms, not just any algorithm.** Golden tests compare topology, positions, normals and UVs. Generic crates (earcut, meshopt simplify, xatlas) give *valid but different* output. Where parity matters, port Blender's algorithm, for example `BLI_polyfill_2d` + beautify for triangulation and `bmesh_decimate_collapse` for Decimate. Use crates only as fallbacks or test references.

---

## 1. glTF / GLB

| Crate | Version | License | Last release | Notes |
|---|---|---|---|---|
| `gltf` | 1.4.1 | MIT/Apache-2.0 | 2024-05-10 | Reader. Repo still gets commits (panic fixes merged 2026-05-11), but there has been no release in 2.5 years and 90 issues are open. `Gltf::from_slice_without_validation` exists (lib.rs:322) and bypasses the `extensionsRequired` rejection. |
| `gltf-json` | 1.4.1 | MIT/Apache-2.0 | 2024-05-10 | Serde structs; can write. Feature-gated extensions: lights_punctual, emissive_strength, ior, pbrSpecularGlossiness, specular, transmission, unlit, variants, volume, texture_transform. Others land in `others: Map<String, Value>` under the `extensions` feature. |
| `myth-gltf` | 1.4.2 | MIT/Apache-2.0 | 2026-03 | One-release fork. Not worth it. |
| `goth-gltf` | 0.3.2 | MIT | 2026-09 | Lightweight reader (nanoserde). Read only. |
| `gltf_kun` | 0.4.0 | MIT/Apache-2.0 | 2026-04 | Graph-based processing on top of `gltf` and bevy_math. Too opinionated. |
| `draco-gltf` | 0.8.0 | Apache-2.0 | 2026-11-11 | Pure Rust. Claims lossless whole-document glTF 2.0 and 2.1-draft round-trip plus Draco. Created 2026-06; young. |

**Pros of gltf/gltf-json:** mature types for core glTF, accessor iterators, widely used (bevy, three-d), good as a **reader cross-check in tests**.
**Cons:** stale releases; extension coverage is about 50% of what frap needs; strict validation on `extensionsRequired`; typed structs drop unknown fields unless the `extensions`/`extras` features are on; its `Index<T>` and `Checked<T>` wrappers make *editing* awkward.

**Recommendation: hand-rolled serde model in our own crate (`frap-gltf`).**
- One struct per glTF object with `#[serde(default, skip_serializing_if = ...)]`.
- `extensions: IndexMap<String, serde_json::Value>` plus typed accessors for known extensions, so unknown extensions round-trip untouched.
- Keep `extras: Option<Box<RawValue>>` (Blender writes custom properties to `extras`).
- Separate concerns: (a) JSON schema structs, (b) GLB container (12-byte header, JSON chunk padded with 0x20 to 4 bytes, BIN chunk padded with 0x00), (c) accessor encode/decode (normalized ints, sparse accessors, `KHR_mesh_quantization` types, byteStride rules), (d) extension codecs (Draco, meshopt, WebP, KTX2).
- Dev-dependency `gltf = "=1.4.1"` as an independent reader. Every frap-written GLB must load with it via `from_slice_without_validation` and with Blender's importer in golden tests. The Khronos glTF-Validator (npm/CLI binary, optional in CI) is the authoritative structure check.
- Keep `draco-gltf` as a **Reference**. Its accessor-contract code is worth reading.

---

## 2. Math

| Crate | Version | License | Notes |
|---|---|---|---|
| `glam` | 0.34.3 (2026-11-08) | MIT/Apache-2.0 | SIMD f32 `Vec3`/`Vec3A`/`Mat4`/`Quat`/`Affine3A`, plus f64 `DVec*`/`DMat*`/`DQuat`. Features: `serde`, `approx-05`, `mint-05`, `zerocopy-08`, `encase-01x` (no `bytemuck` feature in 0.34). MSRV 1.85. |
| `nalgebra` | 0.35.0 (2026-05) | Apache-2.0 | Generic and heavy; slower compiles; strong for linear algebra. |
| `nalgebra-sparse` | 0.12.0 | Apache-2.0 | CSR/CSC plus a basic Cholesky. |

Blender facts: mesh data (`MeshVertex.co`, normals, UVs) and *all* RNA float properties (`Object.location`, `rotation_euler`, `scale`, `matrix_world`, modifier widths) are **C `float` (f32)**. `mathutils` exposes them to Python as doubles but stores f32. Only a few things are double internally (e.g. some scene time values); those can be special-cased.

**Recommendation: Adopt `glam` with f32 as the storage type everywhere, matching Blender bit for bit.** Use `DVec3`/f64 locally inside numerically sensitive algorithms (quadric error matrices, LSCM/ABF solves, plane fits, exact predicates), then round back to f32. Do not adopt nalgebra; `faer` covers sparse and dense solves (section 6).
Pitfall: Euler order. Blender `rotation_mode` is `XYZ` by default, meaning the rotation matrix is `Rz*Ry*Rx`. Implement Blender's `eul_to_mat3`/`mat3_to_eul` (including its compatible-euler logic for keyframing) yourself. `glam::EulerRot` naming differs; do not rely on it without tests.

---

## 3. GPU rendering

| Crate | Version | License | Notes |
|---|---|---|---|
| `wgpu` | **30.0.2** (2026-11-12) | MIT/Apache-2.0 | MSRV 1.87. Backends: `metal`, `vulkan`, `dx12`, `gles`, plus a `noop` feature. Shading languages: `wgsl` (default), `glsl`, `spirv`, `naga-ir`. |
| `naga` | 30.0.2 | MIT/Apache-2.0 | Shader translator bundled with wgpu. Use it directly only for offline shader validation tests. |
| `pollster` | 1.0.1 | Apache-2.0/MIT | `block_on` for wgpu's async adapter and device requests. |

wgpu ships a breaking major roughly every 3 to 6 months (v28 2025-12, v29 2026-03, v30 2026-07). Notable v30 changes: `VertexState.buffers` is now `&[Option<VertexBufferLayout>]`; integer shader I/O must be explicitly `@interpolate(flat)`; `BufferSlice::size()` returns `u64`; zero-size `BufferBinding` is fallible.

Headless offscreen pattern (no window, no surface):
1. `Instance::new(&InstanceDescriptor { backends: Backends::PRIMARY, .. })`, then `request_adapter(&RequestAdapterOptions { compatible_surface: None, power_preference: HighPerformance, force_fallback_adapter: false })`.
2. Render into a `Texture` with usage `RENDER_ATTACHMENT | COPY_SRC`. Use `Rgba16Float`/`Rgba32Float` for the HDR/EEVEE path and tonemap in a final pass (Blender's view transform defaults to **AgX** since 4.0; `Standard` is available). Write `Rgba8UnormSrgb` for the final PNG.
3. `copy_texture_to_buffer` into a `MAP_READ | COPY_DST` buffer. **`bytes_per_row` must be a multiple of `COPY_BYTES_PER_ROW_ALIGNMENT` (256)**; strip the padding per row on readback.
4. `buffer.slice(..).map_async(MapMode::Read, cb)`, then `device.poll(PollType::Wait…)` (check the exact v30 signature), then `get_mapped_range()`.
5. Shadows: depth textures with `Depth32Float` and comparison samplers. IBL: precompute irradiance and prefiltered specular cubemaps plus a BRDF LUT from an equirect HDR in compute shaders at world load.

Platform notes:
- macOS arm64: Metal; robust headless.
- Linux: Vulkan. **For GPU-less CI, use Mesa lavapipe (llvmpipe Vulkan)**, selected via `force_fallback_adapter: true` or `WGPU_ADAPTER_NAME`. This lets render tests run on CI even though users are not required to have a software path.
- Windows: DX12, with the **WARP** software adapter available as a fallback for CI.
- Determinism: GPU output differs across vendors. Render golden tests must use perceptual tolerance (see `image-compare`, section 10), not byte equality.

**Recommendation: Adopt `wgpu = "30"` (`default-features = false`, features `wgsl`, `metal`, `vulkan`, `dx12`) plus `pollster`.** Isolate wgpu behind a `frap-render` crate so the yearly-or-faster major bumps touch one crate only. Hand-write WGSL; do not adopt an engine (bevy, three-d), since the EEVEE and Workbench semantics need full control.

---

## 4. Images and textures

| Crate | Version | License | Notes |
|---|---|---|---|
| `image` | 0.25.10 (2026-03) | MIT/Apache-2.0 | MSRV 1.88. Formats: png, jpeg (zune-jpeg decode, own encoder), webp (decode both kinds; **encode lossless only**), hdr, exr (via `exr`), tga, bmp, dds (decode), tiff, qoi, avif (behind feature). |
| `png` | 0.18.1 | MIT/Apache-2.0 | Direct use for 16-bit PNG and fine control. |
| `exr` | 1.74.3 (2026-11-08) | BSD-3-Clause | Pure Rust OpenEXR read and write, multi-layer. |
| `image-webp` | 0.2.4 | MIT/Apache-2.0 | Pure Rust; decode both kinds, encode lossless only. |
| `webp` | 0.3.1 | MIT/Apache-2.0 | Wraps `libwebp-sys` 0.14.4 (**`cc`-compiled C, no cmake**). Lossy and lossless encode. Needed because Blender's exporter writes lossy WebP at `export_image_quality` (default 75). |
| `zenwebp` | 0.4.6 | **AGPL-3.0** or commercial | **Avoid** (license). |
| `jpeg-encoder` | 0.7.1 | (MIT/Apache) AND IJG | Fast pure Rust JPEG encoder if image's encoder is insufficient. |
| `ktx2` | 0.5.0 (2026-04) | Apache-2.0 | KTX2 **container parser only** (reader). Writing the container is easy by hand (DFD plus level index). |
| `basisu` | 0.1.0 (2026-07) | Apache-2.0 | Pure Rust, bit-exact Basis Universal **transcoder** (ETC1S/UASTC to RGBA/BC). Good for *importing and rendering* `KHR_texture_basisu`. |
| `basisu_c_sys` | 0.9.1 (2026-09) | MIT/Apache-2.0 | Binds Binomial's C API; `encoder` feature. Vendored C++ through `cc`, **but `bindgen` is a build dependency, so libclang is required**. |
| `basis-universal` | 0.3.1 (2023-11) | MIT/Apache-2.0 | Older bindings (cc only, no bindgen); stale upstream (pre-UASTC-HDR). |
| `ktx2-rw` | 0.2.4 | MIT/Apache-2.0 | Wraps libktx; young. Avoid. |

**Recommendation:**
- Adopt `image` (`default-features = false`; features `png`, `jpeg`, `webp`, `hdr`, `exr`, `tga`, `bmp`) and `exr` for multi-layer EXR render output (`scene.render.image_settings.file_format = 'OPEN_EXR'`).
- Adopt `webp` (libwebp) behind a default-on `webp-lossy` feature to match `EXT_texture_webp` export with quality.
- KTX2/Basis: read with `ktx2` and transcode with `basisu` (pure Rust). Encode only with an optional `basisu-encode` cargo feature using `basisu_c_sys` (libclang needed; document it), **off by default**. Since Blender cannot export KTX2, expose it as a frap extension parameter (for example `export_scene.gltf(frap_texture_format='KTX2')`; naming is for the architect).
- Image color management: Blender stores `Image.colorspace_settings.name` (`sRGB`, `Non-Color`, `Linear Rec.709`...). Keep original encoded bytes in `.frap` and in GLB when possible, as Blender does with packed images. Re-encode only when needed.

---

## 5. Geometry compression

### Draco (`KHR_draco_mesh_compression`)
Blender ships Google Draco as `libdraco.dylib` plus `libbf_intern_draco_bridge.dylib` inside the glTF add-on. Its parameter names, verified: `export_draco_mesh_compression_enable`, `_level`, `export_draco_position_quantization`, `_normal_quantization`, `_texcoord_quantization`, `_color_quantization`, `_generic_quantization`.

| Crate | Version | License | Notes |
|---|---|---|---|
| `draco-core` | 2.3.3 (2026-11-01) | Apache-2.0 | **Pure Rust encoder and decoder** (meshes and point clouds). Only depends on bytemuck. CI and fuzzing; claims bitstream compatibility checked against upstream binaries. Created 2026-06, 6 stars, 16 releases. |
| `draco-oxide` | 0.1.0-alpha.12 | MIT/Apache-2.0 | Pure Rust rewrite by Re:Earth (reearth). Alpha; pulls in `gltf`, `image`, `tobj`. |
| `draco_decoder` | 0.0.31 | | Decoder only, WIP. |
| `draco-rs` | 0.1.3 (2025-04) | MIT | FFI to Google Draco, needs **cmake + C++**, rarely used. Avoid. |

**Recommendation: Spike `draco-core` behind a `draco` feature and a frap trait `MeshCompressor`.** Acceptance test: Blender 5.2 imports frap-encoded Draco GLBs, frap decodes Blender-encoded ones, and positions match within the quantization bound. Fallback if it fails: our own `-sys` crate compiling Google Draco's C++ sources with `cc` (no cmake). It is a sizeable but mechanical file list; Draco's `draco/core`, `compression`, `mesh`, `attributes` and `point_cloud` directories build with plain C++17. Do not expect byte-identical output against Blender; compare decoded geometry.

### meshopt (`EXT_meshopt_compression` / `KHR_meshopt_compression`)

| Crate | Version | License | Notes |
|---|---|---|---|
| `meshopt` | **0.7.0** (2026-10-17) | MIT/Apache-2.0 | FFI to **meshoptimizer 1.3** (vendored, **compiled with `cc`**, no cmake, no bindgen at build). Exposes `encode_vertex_buffer[_level]`, `encode_index_buffer`/`_sequence`, `encode_filter_oct/quat/exp`, `simplify*`, optimizers, meshlets. Maintained since 2018; 226 stars. |
| `meshoptimizer-rs` | 0.2.19 | MIT | Pure Rust port claiming byte-identical output with 1.3. Created 2026-10, 2 stars. Reference only. |
| `meshopt-pure`, `optimesh`, `rmeshopt`, `meshopt-rs` | various | | Pure ports, all young or stale. Avoid. |

**Recommendation: Adopt `meshopt = "0.7"`.** It provides the codec for both meshopt extensions, the filters (octahedral normals, quaternion rotations, exponent), and quantization helpers for `KHR_mesh_quantization`. Blender reaches the same features through gltfpack. Map Blender's `export_gltfpack_*` parameters (`tc`, `tq`, `si`, `sa`, `slb`, `vp`, `vt`, `vn`, `vc`, `vpi`, `noq`, `kn`) onto in-process meshopt calls with gltfpack-equivalent semantics. Pitfall: `export_gltfpack_si` (simplification ratio) and texture compression `tc` (KTX2 via basisu in gltfpack) imply simplification and basisu, so feature-gate those consistently.

---

## 6. Mesh algorithms

### 6.1 BMesh-like editable structure: **write our own**
Candidates looked at: `halfedge` 0.3.0, `smesh` 0.2.7, `alum` 0.7.1, `mesh-graph` 0.11, `tri-mesh` 0.6.1 (stale 2024), `redges` 0.3.0 (radial edge, 138k downloads), `plexus`/`hedge` (stale 2019), `baby_shark` 0.3.12.
None model Blender's BMesh semantics: **non-manifold edges through radial loop cycles, wire edges, loose verts, n-gons, per-element flags (select/hide/seam/smooth/sharp), and customdata layers on vert/edge/loop/face.** Half-edge structures cannot represent an edge with 3+ faces. `redges` (radial edge) is closest in concept but generic, and its API does not fit Blender's ops.
**Recommendation:** write `frap-bmesh` mirroring `BMVert`/`BMEdge`/`BMLoop`/`BMFace` with the disk cycle (edge around vert) and radial cycle (loop around edge), stored in index arenas with free lists. `slotmap` 1.1.1 (Zlib) is an option, but plain `Vec` plus a generational or free-list scheme gives better control over Blender-compatible **element index order**, which golden tests need. Convert BMesh ↔ `Mesh` (Blender's flat SoA arrays: `vertices`, `edges`, `loops`, `polygons`, attributes) exactly as Blender's `BM_mesh_bm_from_me` and `BM_mesh_bm_to_me` do, keeping index ordering. Use `smallvec` 1.16.5 for small adjacency lists.

### 6.2 Booleans (Boolean modifier: `FLOAT`, `EXACT`, `MANIFOLD`; edit-mode `mesh.intersect_boolean`)

| Crate | Version | License | Build | Notes |
|---|---|---|---|---|
| `manifold-rust` | 0.19.2 (2026-11-01) | Apache-2.0 | pure Rust (clipper2-rust, dashu) | **Port of Manifold v3.5.4 that passes upstream tests**, plus an exact-rational "robust" mesh-arrangement engine for non-manifold input. Optional rayon, bit-identical results. 36 stars, created 2026-07. |
| `boolmesh` | 0.2.2 (2026-10-27) | **MPL-2.0** | pure Rust (glam) | Manifold-inspired; **carries arbitrary per-corner values** (UVs, colors) through booleans. Requires manifold input. |
| `manifold3d` / `manifold-csg` | 0.4.2 / sys 3.5.106 | MIT/Apache-2.0 | **cmake + git clone at build + TBB** | Official engine via C API. Avoid for build reasons; usable as a CI reference only. |
| `manifold-rs` | 0.7.0 | MIT/Apache-2.0 | cxx + C++ | Avoid. |
| `csgrs` | 0.20.1 (2025-07) | MIT | pure Rust | BSP-tree CSG; not robust, explodes face counts. Avoid. |
| `meshlib` | 3.2.0-29 | (check) | C++ binary | Avoid. |

**Recommendation: Spike `manifold-rust` as the `MANIFOLD` solver** (same algorithm as Blender's backend, so geometry should match closely). Its robust engine is also a candidate for `EXACT`. Blender's EXACT is its own mesh-arrangement code (`mesh_boolean.cc`, also based on Zhou et al. 2016), so the approach matches but the output order will not. Implement `FLOAT` (Blender's BMesh intersect, `bmesh_intersect.cc`) ourselves later, or alias it to EXACT at first. Attribute transfer (UVs, colors, materials, `use_self`, `use_hole_tolerant`) is frap's job in every case: map result faces back to source faces by the IDs Manifold provides. Keep `boolmesh` as a backup (MPL-2.0 is file-level copyleft; acceptable but must be noted).

### 6.3 Triangulation

| Crate | Version | License | Notes |
|---|---|---|---|
| `earcut` (georust) | 0.4.12 (2026-11) | MIT/Apache-2.0 | Fast earcut port; active. |
| `earcutr` | 0.5.0 (2025-05) | ISC | Older port. |
| `spade` | 2.15.1 (2026-03) | MIT/Apache-2.0 | Robust Delaunay and **constrained** Delaunay (CDT), exact predicates. |
| `i_triangle` | 0.51.0 (2026-10) | MIT/Apache-2.0 | Robust integer-based polygon triangulation (holes, self-intersections). Frequent breaking releases. |
| `lyon_tessellation` | 1.0.23 | MIT/Apache-2.0 | 2D path fill; overkill. |

Blender's `Triangulate` modifier (`quad_method`: `BEAUTY`/`FIXED`/`FIXED_ALTERNATE`/`SHORTEST_DIAGONAL`/`LONGEST_DIAGONAL`; `ngon_method`: `BEAUTY`/`CLIP`), `mesh.quads_convert_to_tris`, the glTF exporter's triangulation and `mesh.fill` all use **`BLI_polyfill_2d` (ear clipping) plus `BLI_polyfill_2d_beautify`**. The project/normal plane comes from Newell's method (`axis_dominant_v3_to_m3`).
**Recommendation:** port Blender's polyfill and beautify (about 1k lines of C, GPL; reimplement from the algorithm description and verify against Blender outputs, since frap's license must be chosen deliberately). Adopt `spade` for `mesh.fill`/`beautify_fill`/`grid_fill` helpers and any CDT need. Keep `earcut` as a fallback for degenerate polygons.
**License note:** porting Blender code line by line would make frap GPL-2.0-or-later. The architect must choose frap's license with that in mind; clean-room reimplementation from behavior plus papers keeps MIT/Apache possible.

### 6.4 Subdivision (`SUBSURF` modifier, `object.subdivision_set`, `mesh.subdivide`)

| Crate | Version | License | Notes |
|---|---|---|---|
| `opensubdiv-rs` | 0.7.0 (2026-11-11) | MIT AND LicenseRef-TOST-1.0 | **Pure Rust port of OpenSubdiv Sdc/Vtr/Far plus GPU patch tables**, no deps, `forbid(unsafe_code)`. Catmark/Loop/Bilinear, semi-sharp creases (Uniform/Chaikin), face-varying interpolation, limit evaluation. Created 2026-08; 4 stars, 37 open issues. |
| `opensubdiv-petite` | 0.3.1 | Apache-2.0 | FFI to Pixar C++ (cmake). Avoid. |
| `manifold-rust` | | | Includes "smooth subdivision" (different algorithm). |

Blender's Subdivision Surface modifier settings map 1:1 to OpenSubdiv options: `boundary_smooth` (`ALL`/`PRESERVE_CORNERS`) maps to `VtxBoundaryInterpolation`; `uv_smooth` (`NONE`, `PRESERVE_CORNERS`, `PRESERVE_CORNERS_AND_JUNCTIONS`, `PRESERVE_CORNERS_JUNCTIONS_AND_CONCAVE`, `PRESERVE_BOUNDARIES`, `SMOOTH_ALL`) maps to `FVarLinearInterpolation`; edge `crease` maps to sharpness. `use_limit_surface` and `quality` relate to limit evaluation. Blender's output element ordering (subdiv_mesh.cc, "foreach" traversal) is Blender-specific.
**Recommendation: Spike `opensubdiv-rs`** for the `SUBSURF` modifier. Uniform Catmull-Clark with creases and face-varying UVs is also small enough (about 1.5k lines) to write ourselves against Blender golden output if the crate disappoints. Write our own vertex/face **ordering pass to match Blender's subdiv mesh layout**. `mesh.subdivide` (edit mode, `number_cuts`, `smoothness`, `quadcorner`) is a *different* BMesh algorithm (`bmo_subdivide.cc`); write it ourselves.

### 6.5 Decimation (`DECIMATE`: `COLLAPSE`/`UNSUBDIV`/`DISSOLVE`, verified)
`meshopt::simplify` (attribute-aware, lock border, error metrics) is high quality but **does not match Blender's `COLLAPSE`** (Blender's own quadric edge collapse on triangulated BMesh with symmetry, `vertex_group` factor, `use_collapse_triangulate`). `UNSUBDIV` and `DISSOLVE` (planar, `angle_limit`, `delimit`) are Blender BMesh ops.
**Recommendation:** write all three ourselves (QEM with f64 quadrics in `glam::DMat4`). Use `meshopt::simplify` only for the gltfpack-equivalent `export_gltfpack_si` path.

### 6.6 UV unwrapping and packing
Methods to cover: `uv.unwrap` (`ANGLE_BASED` ABF++, `CONFORMAL` LSCM, `MINIMUM_STRETCH` SLIM; `fill_holes`, `correct_aspect`, `margin_method`, `margin`), `uv.smart_project` (`angle_limit`, `island_margin`, `area_weight`, `correct_aspect`, `scale_to_bounds`), `uv.cube_project`/`cylinder_project`/`sphere_project`/`project_from_view`, `uv.pack_islands`, `uv.mark_seam`, `uv.average_islands_scale`.

| Crate | Version | License | Notes |
|---|---|---|---|
| `faer` | **0.24.4** (2026-06) | MIT | Pure Rust dense **and sparse** LA: sparse Cholesky (LLT/LDLT), LU, QR with AMD/COLAMD orderings. Fast, actively maintained (moved to Codeberg). |
| `nalgebra-sparse` | 0.12.0 | Apache-2.0 | Simpler sparse Cholesky only; slower. |
| `sprs` 0.11.6 / `sprs-ldl` 0.10.0 | MIT/Apache | | sprs-ldl stale (2022). |
| `xatlas-rs-v2` 0.1.4 / `xatlas-rs` 0.1.3 | MIT | | xatlas C++ via `cc` **plus `bindgen` (libclang)**; single release (2025-11) or stale (2019). Avoid. |
| `uvunwrap` | 0.3.0 (2026-10) | MIT | Pure Rust LSCM + ARAP + packing, optional faer. Created 2026-10; 303 downloads. Reference. |

**Recommendation: write our own on `faer` sparse.** LSCM is a least-squares sparse solve (pin two vertices, or use Blender's approach of pinning the two most distant boundary verts). ABF++ is Newton iterations with sparse solves, following Blender's `uvedit_parametrizer.cc` design. SLIM is a local/global iteration with a sparse Cholesky. Smart project and the island packer (Blender 3.6+ packer: `shape_method` `CONCAVE`/`CONVEX`/`AABB`, `rotate_method`, `margin_method`) are Blender-specific heuristics to reimplement. Use `uvunwrap` only as an algorithm reference.

### 6.7 Tangents (MikkTSpace)
| Crate | Version | License | Notes |
|---|---|---|---|
| `bevy_mikktspace` | **1.0.0** (2026-02) | Zlib AND (MIT/Apache-2.0) | Safe Rust port of the reference `mikktspace.c`; very widely used (8.7M downloads). No deps. |
| `mikktspace` (gltf-rs) | 0.3.0 (2022) | MIT/Apache-2.0 | Older port; stale. |

Blender computes tangents with its own C++ port of mikktspace, which reproduces the reference results. Blender's glTF exporter (`export_tangents`) uses `mesh.calc_tangents()`.
**Recommendation: Adopt `bevy_mikktspace = "1"`.** Golden-test tangents against Blender's `export_tangents=True` output, including the w sign.

### 6.8 Other geometry helpers
- Normals (`shade_smooth`, `shade_flat`, `shade_smooth_by_angle` / the 4.1+ "Smooth by Angle" modifier node group, sharp edges, custom split normals): write our own. Since 4.1 Blender has no `use_auto_smooth`; sharpness is the `sharp_edge`/`sharp_face` boolean attributes, and "Smooth by Angle" is a **geometry nodes modifier asset**. frap must decide how to represent it (probably a built-in pseudo-modifier with the same name and inputs).
- BVH and raycasting for knife or snapping: none needed at first; `parry3d` is overkill.
- Hashing for welding (`WELD` modifier, `mesh.remove_doubles` / "Merge by Distance"): own spatial hash; `rustc-hash` 2.1.3 `FxHashMap`.

---

## 7. Parsing the command stream (bpy subset and short form)

| Crate | Version | License | Notes |
|---|---|---|---|
| `logos` | 0.16.1 (2026-01) | MIT/Apache-2.0 | Derive-based fast lexer. |
| `chumsky` | 0.14.1 (2026-11-13) | MIT | Parser combinators with excellent error recovery; heavy generics, slow compiles, API churn between 0.9 and 0.10+. Repo moved to Codeberg. |
| `winnow` | 1.0.5 (2026-11) | MIT | Fast, stable 1.0 combinators (nom successor). |
| `ariadne` 0.6.0 / `miette` 7.6.0 | MIT / Apache-2.0 | | Pretty diagnostics. Not needed for JSON output. |

The grammar is small and LL(1):
- statements: `call`, `assign`, `augassign` (`+=`, `-=`, `*=`), bare expression (a get), `print(expr)`, optional `import bpy`/`import math`/`from mathutils import Vector` as no-ops
- expressions: literals (int, float with exponent, `'..'`/`".."` with escapes, `True`/`False`/`None`), tuple (including `(x,)`), list, dict (needed for some operator args such as `TRANSFORM_OT_translate` value tuples or `bpy.ops.*` with `{'...'}` sets: Blender enum-flag args are **Python sets**, e.g. `mode={'VERT'}`, `type={'FACE'}`), attribute access, subscript (`['Cube']`, `[0]`), calls with positional and keyword args, unary minus, arithmetic (`math.pi/2`, `radians(90)`), constructors such as `Vector((...))`, `Euler((...), 'XYZ')`, `Matrix(...)`, `Color(...)`
- short form: `op.name key=value ...` with comma tuples

**Recommendation: hand-written lexer plus recursive descent (Pratt for arithmetic), zero dependencies.** This gives exact byte spans and tailored, LLM-friendly error messages (for example `"unknown keyword 'sizee' for mesh.primitive_cube_add; did you mean 'size'?"` with a valid-keyword list). It keeps compile times low and makes it trivial to emit structured JSON errors with `line`, `col`, `expected`. `logos` is an acceptable shortcut for the lexer only. Avoid chumsky (compile time, API churn). Use `strsim` (or a simple Levenshtein) for suggestions.
Both front-ends lower into the same AST: `Call { path, args, kwargs }`, `Set { path, value }`, `Get { path }`. They resolve against the typed registry, so a future `mlua` front-end only needs to produce the same IR.

---

## 8. Serialization, `.frap` container, buffers

| Crate | Version | License | Notes |
|---|---|---|---|
| `serde` | 1.0.229 | MIT/Apache-2.0 | |
| `serde_json` | 1.0.154 | MIT/Apache-2.0 | Features `preserve_order` (IndexMap-backed `Map`), `float_roundtrip` (exact parse), `raw_value`. |
| `indexmap` | 2.14.2 | Apache-2.0/MIT | Ordered maps for deterministic output. |
| `ryu` | 1.0.23 | Apache-2.0/BSL-1.0 | serde_json already uses it: shortest round-trip repr. |
| `zip` | **9.6.0** (2026-11-15) | MIT | The zip2 fork; MSRV 1.88. **Ships majors often (v2 to v9 in about 2 years); pin the major.** |
| `flate2` | 1.1.11 | MIT/Apache-2.0 | Backend `zlib-rs` (pure Rust). |
| `bytemuck` | 1.25.2 | Zlib/Apache/MIT | Casting `[f32;3]` slices to bytes for buffers (derive feature). |
| `half` | 2.7.1 | MIT/Apache-2.0 | f16 for float-16 textures and quantized attributes. |
| `base64` | 0.23.1 | MIT/Apache-2.0 | Data URIs in `.gltf` (embedded format). |
| `sha2` | 0.11.1 | MIT/Apache-2.0 | Content-addressed buffer and image names inside `.frap` (dedup, stable diffs). |

Determinism pitfalls for a diffable `scene.json`:
- Never serialize `HashMap`; use `IndexMap` (insertion order = Blender order, e.g. `bpy.data.objects` is name-sorted in Blender!) or `BTreeMap`.
- **f32 values: serialize as f32** (serde_json's `serialize_f32` uses ryu's shortest f32 form: `0.1f32` gives `0.1`, not `0.10000000149011612`). Note that Blender's Python *prints* f32 via double repr (`0.10000000149011612`). frap's JSON responses should print shortest-f32 and document the difference; golden tests compare numerically with tolerance.
- NaN and Inf are not valid JSON; decide on an error vs `null` policy.
- `zip`: use `SimpleFileOptions::default().last_modified_time(fixed DOS epoch 1980-01-01)` and stable entry order for byte-reproducible `.frap`. Use `CompressionMethod::Stored` for already-compressed PNG/JPEG/WebP and Deflated for JSON and raw buffers.
- Large meshes: keep mesh attribute arrays as little-endian binary blobs referenced from `scene.json` by hash (same idea as glTF bufferViews), not inline JSON arrays.

---

## 9. CLI and schema export

| Crate | Version | License | Notes |
|---|---|---|---|
| `clap` | 4.6.7 | MIT/Apache-2.0 | `derive`; flags such as `--describe`, `--format`, `--script file`. |
| `schemars` | 1.2.3 | MIT | JSON Schema draft 2020-12 by default; `derive`, `preserve_order`; supports transforms to inline `$ref`s. |
| `anyhow` 1.0.104 / `thiserror` 2.0.21 | | | `thiserror` for typed library errors (they map to JSON error codes); `anyhow` only in the binary. |
| `tracing` 0.1.44 / `tracing-subscriber` 0.3.23 | MIT | | Logs to **stderr** only; stdout is reserved for the JSON protocol. |

**Recommendation:** the operator and property registry is **runtime data** (it describes Blender RNA: types, enums with `items`, `soft_min`/`soft_max`, `default`, `subtype` such as `TRANSLATION`/`EULER`/`COLOR_GAMMA`, array length, units). Generate `--describe` output **from the registry by hand**, mirroring Blender RNA metadata names (`identifier`, `name`, `description`, `type`, `subtype`, `enum_items`, `default`, `hard_min`...). Use `schemars` only for the static parts: the response envelope, error objects, and `.frap` scene schema. LLM tool-definition consumers (Anthropic and OpenAI tool schemas) support a *subset* of JSON Schema. Provide a flattened per-operator `input_schema` (no `$ref`, `oneOf` kept minimal, enums as `enum`, sets as `array` + `uniqueItems`).
A local Blender is available for verification: generate the registry's names and defaults **from Blender itself** with a script that walks `bpy.ops.<mod>.<op>.get_rna_type().properties` and `bpy.types.*.bl_rna.properties`, dumps them to JSON, and commits the result as a test fixture. That prevents naming drift.

---

## 10. Testing

| Crate | Version | License | Notes |
|---|---|---|---|
| `insta` | 1.49.0 | Apache-2.0 | Snapshots; features `json`, `redactions`, `glob`. Good for `--describe` output and response transcripts. |
| `approx` | 0.5.1 (0.6.0-rc2 exists) | Apache-2.0 | `glam` integrates with the `approx-05` feature. |
| `proptest` | 1.12.0 | MIT/Apache-2.0 | BMesh invariants (Euler characteristic, disk and radial cycle consistency) under random op sequences; parser fuzzing. |
| `trycmd` 1.2.1 / `snapbox` 1.2.2 | MIT/Apache-2.0 | | `.toml`/`.md` transcript tests: stdin script in, JSON lines out. Fits the stream CLI well. |
| `assert_cmd` | 2.2.2 | MIT/Apache-2.0 | Process-level tests that invoke Blender and frap. |
| `rstest` | 0.28.0 | MIT/Apache-2.0 | Parameterized golden cases (one script → many checks). |
| `image-compare` | 0.5.0 (2025-08) | MIT | SSIM/RMS for render goldens across GPU vendors. |

Golden-test harness design notes:
- `BLENDER_BIN` env var, then PATH, then `~/Applications/Blender-5.2.app/Contents/MacOS/Blender`; skip with a visible notice when missing.
- Run the *same* bpy-form script in `blender -b --factory-startup --python-expr` and in frap. Export both to GLB, or dump `mesh.vertices/loops/polygons/uv_layers` as JSON via a Blender helper script, and compare. Topology comparison should be **order-insensitive** where Blender's ordering is not reproduced; positions use tolerance around 1e-5 relative.
- Cache Blender outputs in-repo as fixtures (regenerated with a `--bless` flag) so CI without Blender still runs the comparisons against stored Blender truth.

---

## 11. Future Lua front-end

| Crate | Version | License | Notes |
|---|---|---|---|
| `mlua` | 0.12.2 (2026-10) | MIT | Lua 5.1 to 5.5, LuaJIT, Luau. `vendored` compiles the C sources with `cc` (no cmake). Luau is C++ through `cc` and also fine. `serde` feature maps registry values. MSRV 1.88. |

**Recommendation:** no dependency now. Keep the registry API (`invoke(op_id, args: Map<String, Value>) -> Result<Value>` and `get(path)`/`set(path, value)`) free of parser types so an `mlua` front-end can bind `bpy.ops.*` and `bpy.data` userdata to it later. Prefer `lua54` + `vendored`. Choose `luau` if sandboxing for agents matters (Luau has a safe-env mode).

---

## 12. Build-complexity matrix (targets: macOS arm64, Linux x86_64/arm64, Windows x86_64 MSVC)

| Dependency | Native code | Toolchain needed | Verdict |
|---|---|---|---|
| wgpu | none (system Metal/Vulkan/D3D12) | — | OK |
| meshopt | C++ (vendored) | `cc` + C++ compiler | OK on all three |
| webp / libwebp-sys | C (vendored) | `cc` + C compiler | OK |
| mlua (vendored) | C / C++ | `cc` | OK (future) |
| draco-core, manifold-rust, opensubdiv-rs, bevy_mikktspace, faer, image, exr | pure Rust | — | OK |
| basisu_c_sys (`encoder`) | C++ (vendored) + **bindgen** | `cc` + **libclang** | Optional feature only |
| xatlas-rs(-v2) | C++ + **bindgen** | libclang | Avoid |
| manifold3d / manifold-csg-sys | C++ | **cmake + git (clone at build) + TBB** | Avoid |
| draco-rs, opensubdiv-petite | C++ | **cmake** | Avoid |

Rule: **the default feature set must build with only `cargo` plus a system C/C++ compiler** (Xcode CLT, gcc/clang, MSVC Build Tools). No cmake, no libclang, no network access in `build.rs`.

---

## 13. Recommended `Cargo.toml` dependency table

Versions are the latest stable on crates.io as of 2026-11-16. Use a workspace `[workspace.dependencies]` table, `rust-version = "1.97"` (well above the highest MSRV seen, image/zip/mlua at 1.88), and commit `Cargo.lock`.

| Crate | Version req | Features / notes | Status | Purpose |
|---|---|---|---|---|
| `glam` | `0.34.3` | `serde`, `approx-05` (no `bytemuck` feature in 0.34; see note) | Adopt | f32 math (DVec in algorithms) |
| `wgpu` | `30.0.2` | `default-features=false`, `wgsl`, `metal`, `vulkan`, `dx12`, `std` | Adopt | GPU render |
| `pollster` | `1.0.1` | | Adopt | block on wgpu futures |
| `image` | `0.25.10` | `default-features=false`, `png`,`jpeg`,`webp`,`hdr`,`exr`,`tga`,`bmp` | Adopt | image I/O |
| `png` | `0.18.1` | | Adopt | 16-bit render output |
| `exr` | `1.74.3` | | Adopt | multilayer EXR |
| `webp` | `0.3.1` | optional feature `webp-lossy` (default on) | Adopt | lossy EXT_texture_webp |
| `ktx2` | `0.5.0` | | Adopt | read KTX2 |
| `basisu` | `0.1.0` | optional `basisu` | Spike | transcode KTX2/Basis for render/import |
| `basisu_c_sys` | `0.9.1` | optional `basisu-encode`, `encoder` | Optional | encode KHR_texture_basisu (needs libclang) |
| `meshopt` | `0.7.0` | | Adopt | meshopt codec, filters, simplify |
| `draco-core` | `=2.3.3` | optional `draco` (default on after spike passes) | Spike | KHR_draco_mesh_compression |
| `manifold-rust` | `=0.19.2` | `parallel` optional | Spike | Boolean MANIFOLD/EXACT |
| `opensubdiv-rs` | `=0.7.0` | | Spike | SUBSURF modifier |
| `bevy_mikktspace` | `1.0.0` | | Adopt | tangents |
| `spade` | `2.15.1` | | Adopt | CDT for fill ops |
| `earcut` | `0.4.12` | | Adopt (fallback) | ngon triangulation fallback |
| `faer` | `0.24.4` | `default-features=false`, `std`, `linalg`, `sparse`, `sparse-linalg`, `rayon` | Adopt | LSCM/ABF/SLIM solvers |
| `smallvec` | `1.16.5` | `union`, `const_generics` | Adopt | adjacency lists |
| `rustc-hash` | `2.1.3` | | Adopt | fast internal hashing |
| `serde` | `1.0.229` | `derive` | Adopt | |
| `serde_json` | `1.0.154` | `preserve_order`, `float_roundtrip`, `raw_value` | Adopt | protocol, glTF JSON, scene.json |
| `indexmap` | `2.14.2` | `serde` | Adopt | ordered maps |
| `zip` | `9.6.0` | `default-features=false`, `deflate-flate2-zlib-rs` | Adopt | `.frap` container |
| `bytemuck` | `1.25.2` | `derive`, `extern_crate_alloc` | Adopt | buffer casting |
| `half` | `2.7.1` | `bytemuck` | Adopt | f16 |
| `base64` | `0.23.1` | | Adopt | data URIs |
| `sha2` | `0.11.1` | | Adopt | content hashes in `.frap` |
| `clap` | `4.6.7` | `derive`, `env` | Adopt | CLI |
| `schemars` | `1.2.3` | `derive`, `preserve_order` | Adopt | static JSON Schemas |
| `thiserror` | `2.0.21` | | Adopt | typed errors |
| `anyhow` | `1.0.104` | | Adopt (bin only) | |
| `tracing` / `tracing-subscriber` | `0.1.44` / `0.3.23` | stderr writer, `env-filter` | Adopt | diagnostics |
| `logos` | `0.16.1` | | Optional | lexer (or hand-written) |
| `mlua` | `0.12.2` | `lua54`, `vendored`, `serde` | Future | Lua front-end |
| **dev:** `insta` | `1.49.0` | `json`, `redactions`, `glob` | Adopt | snapshots |
| **dev:** `approx` | `0.5.1` | | Adopt | float asserts |
| **dev:** `proptest` | `1.12.0` | | Adopt | BMesh invariants, parser fuzz |
| **dev:** `trycmd` | `1.2.1` | | Adopt | stdin→stdout transcripts |
| **dev:** `assert_cmd` | `2.2.2` | | Adopt | Blender/frap process tests |
| **dev:** `rstest` | `0.28.0` | | Adopt | parameterized goldens |
| **dev:** `image-compare` | `0.5.0` | | Adopt | render goldens (SSIM) |
| **dev:** `gltf` | `=1.4.1` | `extensions`, `extras`, `KHR_*` features | Reference | independent GLB reader check |

Note: glam 0.34.3 has **no `bytemuck` feature** (verified: it offers `zerocopy-08`, `encase-012/013`, `rkyv-08`, `mint-05`). Store mesh attributes as `Vec<[f32; 3]>` (which bytemuck casts natively) and convert to and from `glam::Vec3` at algorithm boundaries, or enable `zerocopy-08`.

Explicitly **not** recommended: `nalgebra` (redundant with glam and faer), `gltf-json` as the write model, `chumsky`, `csgrs`, `manifold3d`/`manifold-rs` (C++ build), `draco-rs`, `opensubdiv-petite`, `xatlas-rs*`, `zenwebp` (AGPL), `meshopt-rs` and other young pure meshopt ports, any halfedge crate as the core mesh.

---

## 14. Open questions for the architect

1. **License of frap.** It decides whether Blender code may be ported directly (GPL) or must be reimplemented clean-room. This is the biggest factor for parity of triangulation, decimation, smart_project, the UV packer, bevel, and the subsurf ordering.
2. **Element order parity.** Should goldens require Blender's exact vertex and face index order (needed for `bpy.data.meshes['X'].vertices[5].co` scripts to behave identically), or only geometric equivalence? Index-stable ops (primitives, extrude, inset, loopcut) should match exactly. Booleans and subsurf may only match geometrically.
3. **f32 print policy** in JSON responses: shortest-f32 (recommended) vs Python double repr.
4. **Smooth by Angle** is a geometry-nodes asset in Blender 4.1+. Should frap emulate it as a pseudo-modifier with an identical name and inputs?
5. **wgpu upgrade cadence:** pin to 30.x and upgrade at most twice a year.
6. **Young pure-Rust ports** (`draco-core`, `manifold-rust`, `opensubdiv-rs`): consider vendoring or forking into the workspace if upstream maintenance stalls. All are Apache/MIT compatible.
