# glTF 2.0 / GLB and Blender 5.2 `io_scene_gltf2`: research notes for frap

Status: research only. No Rust code is proposed here. Date: 2026-11-16.

## 0. Sources and how much to trust each claim

| Tag | Meaning |
|---|---|
| **[V-src]** | Read directly in the bundled add-on source: `~/Applications/Blender-5.2.app/Contents/Resources/5.2/scripts/addons_core/io_scene_gltf2`, `bl_info.version = (5, 2, 41)`, Blender `5.2.4 LTS` (hash cd5dc428cce1). |
| **[V-run]** | Seen in a real export or import run with `blender -b --factory-startup` (the scripts are summarised in Appendix B). |
| **[V-rna]** | Dumped from `bpy.ops.export_scene.gltf.get_rna_type().properties` (and the import equivalent) on 5.2.4. |
| **[Spec]** | glTF 2.0 spec (2.0.1 revision) and the Khronos extension READMEs, from memory and checked against the Khronos registry README in Nov 2026. |
| **[?]** | Uncertain. Verify before you depend on it. |

The export operator has **110 RNA properties** and the import operator has **20** **[V-rna]**. Section 9 lists all of them.

---

## 1. GLB container: byte layout [Spec] (and how Blender writes it [V-src][V-run])

Everything is little-endian. The file is laid out as follows:

```
offset 0   uint32 magic   = 0x46546C67  ("glTF")
offset 4   uint32 version = 2
offset 8   uint32 length  = total file length in bytes (header + all chunks, padding included)
offset 12  Chunk 0: uint32 chunkLength, uint32 chunkType = 0x4E4F534A ("JSON"), chunkData[chunkLength]
           Chunk 1 (optional): uint32 chunkLength, uint32 chunkType = 0x004E4942 ("BIN\0"), chunkData
           ... further chunks of unknown type: readers MUST ignore them
```

Rules:
- The JSON chunk MUST come first. At most one BIN chunk is allowed, and it MUST be the second chunk.
- Each chunk starts and ends on a 4-byte boundary. Pad the JSON chunk with trailing **spaces (0x20)** and the BIN chunk with **zeros (0x00)**. `chunkLength` includes the padding.
- The JSON is UTF-8 with no BOM.
- `buffers[0]` with **no `uri`** refers to the BIN chunk. Its `byteLength` may be up to 3 bytes smaller than the BIN chunk because of padding. No other buffer may omit `uri` (except the meshopt fallback buffer, see §3.9).
- In a GLB, images are normally stored as `image.bufferView` + `image.mimeType`, not as data URIs.

Blender's writer (`io/exp/export.py`) **[V-src][V-run]**:
- It writes the header, then JSON, then BIN. It omits the BIN chunk when the binary data is empty.
- JSON is compact (`separators=(',', ':')`, no indent, `allow_nan=False`). `.gltf` output uses tab indentation instead.
- Top-level keys are sorted in a fixed order: `asset, extensionsUsed, extensionsRequired, extensions, extras, scene, scenes, nodes, cameras, animations, materials, meshes, textures, images, skins, accessors, bufferViews, samplers, buffers`.
- The default cube scene gives a 1936-byte file: JSON 1068 B, BIN 840 B **[V-run]**.
- `asset.generator = "Khronos glTF Blender I/O v5.2.41"`.
- Floats: `__fix_json` turns any float that is an integer value into an int (`1.0` → `1`), which avoids `INTEGER_WRITTEN_AS_FLOAT` validator warnings. Other floats are float32 values widened to double and printed with Python `repr`, for example `0.800000011920929`.
  - Quirk: tuples are not recursed into, so some values still come out as `1.0`. One example is `sheenColorFactor`.
  - For JSON comparisons, compare numbers with a tolerance. Never compare text.
- Empty dicts and lists are dropped. `None` is dropped.
- An extension that has an empty object is dropped unless it is animated. The exception is `KHR_materials_unlit`, which is always `{}`.

## 2. glTF 2.0 core: what frap must write and read [Spec]

### 2.1 Global conventions
- Units are **metres** and **radians**. The coordinate system is right-handed with **+Y up**. The front of an asset faces **+Z**. Cameras and lights look along **-Z**, with +Y up and +X right.
- Front faces wind **counter-clockwise**. When a node's global transform has a **negative determinant**, the winding flips. Blender handles this, so frap's exporter does not need to flip indices for mirrored transforms.
- UV origin: glTF (0,0) is the **top-left** of the image, while Blender's (0,0) is the bottom-left. Convert with `v_gltf = 1 - v_blender` **[V-src]**.
- Colours: `baseColorFactor`, `emissiveFactor`, vertex `COLOR_n` and light colours are **linear**. `baseColorTexture`, `emissiveTexture`, `specularColorTexture` and `sheenColorTexture` are sRGB-encoded. All other textures are linear.
- Every object is referenced by index. Names are optional and need not be unique. If an array is present it MUST NOT be empty.
- `asset.version = "2.0"` is required. `asset.minVersion`, `asset.generator` and `asset.copyright` are optional. Blender writes `copyright` from `export_copyright`.
- `extensionsUsed` lists every extension that appears anywhere in the file. `extensionsRequired` is the subset a loader must support.
- `extras` may appear on any object and holds application JSON. `extensions` is an object keyed by extension name.

### 2.2 Buffers, bufferViews, accessors
- `buffer {byteLength, uri?}`. In `.gltf` files the `uri` may be a `data:application/octet-stream;base64,` URI or a relative path.
- `bufferView {buffer, byteOffset=0, byteLength>=1, byteStride?, target?}`:
  - `target` is `34962` (ARRAY_BUFFER) for vertex attributes and `34963` (ELEMENT_ARRAY_BUFFER) for indices. Leave it out for animation data, inverse bind matrices, images and sparse data.
  - `byteStride` is only allowed on vertex-attribute views. It must be between 4 and 252 and a multiple of 4. It is **required when two or more attribute accessors share (interleave) one view**. When it is absent, data is tightly packed.
- `accessor {bufferView?, byteOffset=0, componentType, normalized=false, count>=1, type, min?, max?, sparse?}`:
  - `componentType`: 5120 BYTE, 5121 UNSIGNED_BYTE, 5122 SHORT, 5123 UNSIGNED_SHORT, 5125 UNSIGNED_INT, 5126 FLOAT. 5124 (INT) is **not** allowed.
  - `type`: SCALAR, VEC2, VEC3, VEC4, MAT2, MAT3, MAT4. Matrices are **column-major**. Each matrix column must start on a 4-byte boundary, which means MAT2 and MAT3 of BYTE, and MAT3 of SHORT, need padding.
  - Alignment: `accessor.byteOffset` and `accessor.byteOffset + bufferView.byteOffset` must be multiples of the component size. For vertex attributes, each element must also start on a 4-byte boundary.
  - `normalized` is valid only for integer types other than UNSIGNED_INT, and never for indices. Decoding: ubyte `c/255`, byte `max(c/127,-1)`, ushort `c/65535`, short `max(c/32767,-1)`. Encoding uses `round(f*255)` and so on.
  - `min`/`max` are **required for POSITION** accessors and for **animation sampler `input`** accessors. They hold one value per component, and for a sparse accessor they describe the data after substitution.
  - If `bufferView` is absent, the data is all zeros. In that case the accessor must have `sparse` or be meant as zeros.
- `sparse {count, indices{bufferView, byteOffset, componentType ∈ {5121,5123,5125}}, values{bufferView, byteOffset}}`:
  - Indices must be strictly increasing.
  - Index and value bufferViews must have no `target` and no `byteStride`.
  - The base data comes from `accessor.bufferView`, or is zeros when there is none. That is the usual pattern for morph targets.

### 2.3 Meshes and primitives
- `mesh {primitives[>=1], weights?, name?, extras?}`.
- `primitive {attributes{NAME: accessor}, indices?, material?, mode=4, targets?}`.
- `mode`: 0 POINTS, 1 LINES, 2 LINE_LOOP, 3 LINE_STRIP, 4 TRIANGLES (the default), 5 TRIANGLE_STRIP, 6 TRIANGLE_FAN.
- `indices`: SCALAR of UNSIGNED_BYTE, UNSIGNED_SHORT or UNSIGNED_INT, with no `normalized` and no stride. The **maximum value of the type (the primitive restart value) MUST NOT be used** (2.0.1). Without indices, the primitive draws `count` vertices in order.
- All attribute accessors in a primitive must have the same `count`.

Standard attributes and their allowed component types (the core spec, before `KHR_mesh_quantization` widens them):

| Attribute | Type | Component types | Notes |
|---|---|---|---|
| POSITION | VEC3 | FLOAT | min/max required |
| NORMAL | VEC3 | FLOAT | unit length. If missing, the client computes flat normals |
| TANGENT | VEC4 | FLOAT | xyz unit length, w = ±1 (bitangent sign). If missing, MikkTSpace is recommended |
| TEXCOORD_n | VEC2 | FLOAT / UBYTE norm / USHORT norm | sets must be numbered contiguously from 0 |
| COLOR_n | VEC3/VEC4 | FLOAT / UBYTE norm / USHORT norm | linear; multiplies the base colour |
| JOINTS_n | VEC4 | UBYTE / USHORT | indices into `skin.joints` |
| WEIGHTS_n | VEC4 | FLOAT / UBYTE norm / USHORT norm | weights sum to 1 across all sets, and the same joint must not appear twice with non-zero weight |
| `_CUSTOM` | any | any except UINT unless normalized | application specific; Blender exports these only with `export_attributes` and a leading `_` |

### 2.4 Morph targets
- `primitive.targets = [{POSITION?, NORMAL?, TANGENT?, TEXCOORD_n?, COLOR_n?}]`. TEXCOORD and COLOR targets were added in 2.0.1. Every value is a **delta**.
- TANGENT deltas are **VEC3**, with no w.
- POSITION target accessors MUST have min/max.
- **Every primitive in a mesh must have the same number of targets, in the same order.**
- `mesh.weights` holds the default weights. `node.weights` overrides them per node, and animation path `weights` animates them.
- Target names are not part of the core spec. The de facto convention is **`mesh.extras.targetNames: [string]`**, which Blender writes and reads **[V-src]**.

### 2.5 Skins
- `skin {inverseBindMatrices?, joints[>=1], skeleton?, name?}`:
  - IBM is a MAT4 FLOAT accessor with count equal to the number of joints. When it is absent, every IBM is the identity.
  - Joint matrix = `globalTransform(joint) * IBM[i]`.
- The **global transform of the node that holds the skinned mesh MUST be ignored**. Only the joint transforms apply.
- A node with `skin` must have `mesh`. That mesh's primitives must include JOINTS_0 and WEIGHTS_0, with the same number of sets for each.
- `skeleton` is optional and only a hint (the common root). Blender never writes it **[V-src]**.

### 2.6 Nodes and scenes
- A node has either `matrix` (column-major, must decompose to TRS, and cannot be animated) or `translation`, `rotation` (quaternion **xyzw**, unit length) and `scale`.
- Other node fields: `children`, `mesh`, `skin`, `camera`, `weights`.
- The node graph must be a set of **disjoint trees**: no node may have two parents.
- `scene.nodes` lists root nodes only. `scene` names the default scene.

### 2.7 Cameras
- `perspective {yfov (rad, >0), znear (>0), zfar? (> znear; if absent the projection is infinite), aspectRatio?}`. When `aspectRatio` is absent, the viewport's aspect ratio is used.
- `orthographic {xmag, ymag (non-zero; half extents), znear (>=0), zfar (> znear)}`.

### 2.8 Materials, textures, samplers, images
- `material`:
  - `pbrMetallicRoughness {baseColorFactor=[1,1,1,1], baseColorTexture, metallicFactor=1, roughnessFactor=1, metallicRoughnessTexture}`. In `metallicRoughnessTexture`, **G = roughness and B = metallic**.
  - `normalTexture{scale=1}`: tangent space, OpenGL convention (+Y), XYZ = 2*RGB - 1.
  - `occlusionTexture{strength=1}`: occlusion is in the **R** channel.
  - `emissiveTexture`, `emissiveFactor=[0,0,0]`.
  - `alphaMode ∈ {OPAQUE (default), MASK, BLEND}`. `alphaCutoff=0.5` applies only with MASK.
  - `doubleSided=false`.
  - When a primitive has no material, the client uses the default material.
- `textureInfo {index, texCoord=0}`, plus `scale` for normal textures and `strength` for occlusion textures.
- `texture {sampler?, source?}`. `source` may be omitted when an extension supplies the image (WebP or BasisU).
- `image {uri | (bufferView + mimeType)}`. `mimeType` is **required** with `bufferView`. The core types are `image/png` and `image/jpeg`.
- `sampler`:
  - magFilter: 9728 NEAREST, 9729 LINEAR.
  - minFilter: 9728, 9729, 9984 NEAREST_MIPMAP_NEAREST, 9985 LINEAR_MIPMAP_NEAREST, 9986 NEAREST_MIPMAP_LINEAR, 9987 LINEAR_MIPMAP_LINEAR.
  - wrapS/wrapT: 33071 CLAMP_TO_EDGE, 33648 MIRRORED_REPEAT, 10497 REPEAT (the default).

### 2.9 Animations
- `animation {channels[>=1], samplers[>=1], name?}`.
- `channel {sampler, target{node?, path ∈ translation|rotation|scale|weights|pointer}}`. `pointer` comes from KHR_animation_pointer, and in that case `node` is omitted.
- Within one animation, no two channels may target the same node and path.
- `sampler {input, output, interpolation = LINEAR|STEP|CUBICSPLINE}`:
  - `input` is SCALAR FLOAT seconds, strictly increasing, with **min/max required**. Times do not have to start at 0.
  - `output`: translation and scale are VEC3 FLOAT. Rotation is VEC4 FLOAT, or normalized BYTE/UBYTE/SHORT/USHORT. Weights are SCALAR with count × the number of targets.
  - LINEAR on rotation means **slerp**.
- **CUBICSPLINE layout**:
  - Output count is `3 × input.count`. Each keyframe stores `[inTangent a_k, value v_k, outTangent b_k]`. For `weights`, each keyframe stores all target in-tangents, then all values, then all out-tangents.
  - Tangents are derivatives **per second**: the curve uses `m_k = (t_{k+1}-t_k)·b_k`.
  - At least 2 keyframes are required.
  - Quaternion results must be normalised after evaluation.

## 3. Extensions (status checked against the Khronos registry README, Nov 2026)

**Ratified (Khronos):**
- KHR_animation_pointer
- KHR_draco_mesh_compression
- KHR_lights_punctual
- KHR_materials_anisotropy, clearcoat, dispersion, emissive_strength, ior, iridescence, sheen, specular, transmission, unlit, variants, volume
- KHR_mesh_quantization
- KHR_texture_basisu
- KHR_texture_transform
- KHR_xmp_json_ld
- KHR_node_visibility, KHR_node_selectability, KHR_node_hoverability
- KHR_interactivity
- KHR_gaussian_splatting

**Multi-vendor, complete:** EXT_meshopt_compression, EXT_texture_webp, EXT_mesh_gpu_instancing.

**Not ratified:**
- **Release candidate:** KHR_materials_diffuse_transmission.
- **Drafts:** KHR_accessor_float64, KHR_collision_shapes, KHR_physics_rigid_bodies, KHR_materials_subsurface, KHR_texture_procedurals, KHR_texture_video, KHR_audio_graph.

**KHR_meshopt_compression [?]:** Blender 5.2 can already export it (`export_meshopt_extension`). A Khronos PR started it in July 2025 as a copy of the EXT spec, and gltfpack emits it with `-cz`. I could not confirm that it is ratified, so treat it as optional and off by default.

**Archived:** KHR_materials_pbrSpecularGlossiness. Blender still *imports* it.

Schemas below. All factors are optional, and the default is shown after `=`.

### 3.1 KHR_lights_punctual
- Root: `extensions.KHR_lights_punctual.lights = [{type: directional|point|spot, color=[1,1,1], intensity=1, range?, spot{innerConeAngle=0, outerConeAngle=π/4}, name?}]`.
- Node: `extensions.KHR_lights_punctual = {light: idx}`.
- **Units: point and spot use candela (lm/sr). Directional uses lux (lm/m²).**
- Falloff is inverse-square. `range` (> 0) is the distance at which the light should reach zero, and should be applied as a smooth window. It is not allowed on directional lights.
- `0 ≤ inner < outer ≤ π/2`. The light points along local -Z.
- Blender puts this extension in **extensionsRequired** as well as extensionsUsed **[V-run]**.

### 3.2 Material extensions (all inside `material.extensions`)

| Extension | Properties (defaults) | Texture channels |
|---|---|---|
| KHR_materials_emissive_strength | emissiveStrength=1 | — |
| KHR_materials_ior | ior=1.5 (0 or ≥1) | — |
| KHR_materials_transmission | transmissionFactor=0, transmissionTexture | R |
| KHR_materials_volume | thicknessFactor=0, thicknessTexture, attenuationDistance=+∞, attenuationColor=[1,1,1] | thickness in G. Needs transmission or diffuse_transmission |
| KHR_materials_specular | specularFactor=1, specularTexture, specularColorFactor=[1,1,1], specularColorTexture | specular in **A**; colour RGB (sRGB) |
| KHR_materials_clearcoat | clearcoatFactor=0, clearcoatTexture, clearcoatRoughnessFactor=0, clearcoatRoughnessTexture, clearcoatNormalTexture | factor R, roughness G |
| KHR_materials_sheen | sheenColorFactor=[0,0,0], sheenColorTexture, sheenRoughnessFactor=0, sheenRoughnessTexture | colour RGB (sRGB), roughness **A** |
| KHR_materials_iridescence | iridescenceFactor=0, iridescenceTexture, iridescenceIor=1.3, iridescenceThicknessMinimum=100, iridescenceThicknessMaximum=400 (nm), iridescenceThicknessTexture | factor R, thickness G |
| KHR_materials_anisotropy | anisotropyStrength=0, anisotropyRotation=0 (rad), anisotropyTexture | RG = direction ([-1,1] encoded), B = strength |
| KHR_materials_dispersion | dispersion=0 (= 20/Abbe number) | — needs volume |
| KHR_materials_unlit | `{}` | — |
| KHR_materials_diffuse_transmission (RC) | diffuseTransmissionFactor=0, diffuseTransmissionTexture, diffuseTransmissionColorFactor=[1,1,1], diffuseTransmissionColorTexture | factor A, colour RGB |
| KHR_materials_variants | root `{variants:[{name}]}`; primitive `{mappings:[{material, variants:[i], name?}]}` | — |

### 3.3 KHR_texture_transform
- Location: `textureInfo.extensions.KHR_texture_transform = {offset=[0,0], rotation=0, scale=[1,1], texCoord?}`.
- UV' = T·R·S·UV with R = [[cos, sin], [-sin, cos]]. The rotation is counter-clockwise in UV space.
- `texCoord`, when present, overrides `textureInfo.texCoord`.
- Blender lists this extension as **required** **[V-run]**.

### 3.4 KHR_mesh_quantization
- Widens the allowed component types:
  - POSITION: BYTE, UBYTE, SHORT or USHORT, normalized or not.
  - NORMAL: BYTE or SHORT normalized.
  - TANGENT: BYTE or SHORT normalized.
  - TEXCOORD: BYTE, UBYTE, SHORT or USHORT, normalized or not.
  - Morph targets follow the same rules.
- Each element is still 4-byte aligned, so a VEC3 of SHORT needs stride 8.
- Dequantisation usually lives in the node transform.
- Must be listed as required.
- The Blender exporter **never writes it on its own**. Only the gltfpack post-process emits it. The importer accepts it **[V-src]**.

### 3.5 KHR_draco_mesh_compression
- `primitive.extensions.KHR_draco_mesh_compression = {bufferView, attributes:{POSITION: dracoAttrId, ...}}`.
- Accessors keep `count`, `componentType`, `type` and `min`/`max`. They have **no bufferView** unless an uncompressed fallback is supplied.
- Only for TRIANGLES and TRIANGLE_STRIP.
- Blender adds it to used and **required** **[V-src]**.
- Blender parameters (§9): level 0–10 (default 6). Quantisation bits: position 14, normal 10, texcoord 12, color 10, generic 12.

### 3.6 EXT_meshopt_compression (and KHR_meshopt_compression)
- `bufferView.extensions.EXT_meshopt_compression = {buffer, byteOffset, byteLength, byteStride, count, mode: ATTRIBUTES|TRIANGLES|INDICES, filter: NONE|OCTAHEDRAL|QUATERNION|EXPONENTIAL}`.
- The parent bufferView points at a **fallback buffer** that has no `uri` and carries `extensions.EXT_meshopt_compression.fallback = true`.
- In a GLB, Blender puts the compressed data in buffer 0 (the BIN chunk) and the fallback in buffer 1 **[V-src]**.
- KHR_meshopt_compression is the same scheme. As I understand it, it also adds the v1 encoders and a COLOR filter **[?]**.

### 3.7 Texture-format extensions
- **EXT_texture_webp**: `texture.extensions.EXT_texture_webp = {source}`. `texture.source` is an optional PNG/JPEG fallback. If there is no fallback, mark the extension required.
- **KHR_texture_basisu**: `texture.extensions.KHR_texture_basisu = {source}` pointing at an image with `mimeType: image/ktx2` (KTX2 with ETC1S/BasisLZ or UASTC). Width and height should be multiples of 4.
  - The **Blender 5.2 exporter and importer do not support it natively** **[V-src]**. KTX2 only appears through gltfpack `-tc`, and that needs an external gltfpack binary.
  - For frap: implement it as optional through the `basis-universal` crate (C++ FFI), or leave it out at first.

### 3.8 EXT_mesh_gpu_instancing
- `node.extensions.EXT_mesh_gpu_instancing.attributes = {TRANSLATION, ROTATION, SCALE, _CUSTOM}`.
- Blender writes it when `export_gpu_instances` is on, for children of an Empty.

### 3.9 KHR_animation_pointer (ratified)
- `channel.target = {path: "pointer", extensions:{KHR_animation_pointer:{pointer:"/materials/0/pbrMetallicRoughness/baseColorFactor"}}}`, with no `node`.
- Any property the spec marks animatable can be targeted, including extension properties such as `/extensions/KHR_lights_punctual/lights/0/intensity` or `/cameras/0/perspective/yfov`.
- Blender exports it only when **`export_pointer_animation=True` *and* the animation mode is NLA_TRACKS or SCENE**. `export_convert_animation_pointer` additionally routes TRS and weights through pointers **[V-src]**.

### 3.10 KHR_node_visibility (ratified)
- `node.extensions.KHR_node_visibility = {visible: true}`. A hidden node hides its whole subtree.
- Animatable through the pointer `/nodes/i/extensions/KHR_node_visibility/visible`. Boolean animation uses STEP, but I have not confirmed the output component type **[?]**.
- **Not implemented by Blender 5.2's add-on** in either direction **[V-src]**.
- frap could map it from `hide_render` as a frap extension. It must be off by default to stay identical to Blender.

### 3.11 extras
- Blender writes `extras` only when `export_extras=True`. The one exception is `mesh.extras.targetNames`, which it always writes when shape keys are exported.
- Sources are an ID's custom properties plus system ID properties (`bl_system_properties_get()`). Only JSON-convertible values are kept.
- These keys are never exported: `cycles`, `cycles_visibility`, `cycles_curves`, `glTF2ExportSettings`, `gltf2_mesh_applied`, the `gltf2_*variant*` keys **[V-src]**.
- extras are written on objects (nodes), mesh data, materials, cameras, lights and the scene.

---
## 4. Blender → glTF export pipeline (io_scene_gltf2 5.2.41)

### 4.0 Order of operations [V-src]
1. If the active object is in a mode other than OBJECT, the exporter calls `object.mode_set(mode='OBJECT')`.
2. **Unless `export_current_frame` is set, it calls `scene.frame_set(0)`.** Frame **0**, not `frame_start`, defines the rest TRS of every object. It restores the original frame afterwards.
   - Pitfall: an object keyed from frame 1 onward exports its frame-0 value, which is the constant extrapolation of the first key.
3. It builds a virtual export tree, the "vtree":
   - Roots are scene objects with no parent, **sorted by natural, case-insensitive name**, the same order as the Outliner (`Object2` sorts before `Object10`) **[V-run]**.
   - Filters then apply: selection, visible, renderable, active collection, collection, `glTF_not_exported` collection, cameras, lights, deform bones.
4. It gathers nodes, meshes, materials, skins and animations. Each binary blob is appended to the single buffer in the order it is gathered.
5. It removes extensions that are empty and not animated, applies the special cases for ior, volume, iridescence and dispersion, prunes `extensionsUsed` and `extensionsRequired`, and writes the file.

**Node array order:** in the run, the `nodes` array came out post-order, children before their parents: `[Mover, Tip, Root, Skinned, Rig]` **[V-run]**. `children` arrays list objects and bones in vtree order. For example, the armature had `"children":[3,2]`, mesh first and then the root bone.
- Golden tests must compare nodes **by name and by hierarchy, not by index**.

### 4.1 Coordinate conversion (`export_yup=True`, the default) [V-src][V-run]

| Data | Blender (Z-up) → glTF (Y-up) |
|---|---|
| location / vertex position / normal / tangent xyz / morph deltas | (x, y, z) → (x, z, **-y**) |
| quaternion (w, x, y, z) | → (w, x, z, -y), written as xyzw `[x, z, -y, w]` |
| scale (x, y, z) | → (x, z, y) |
| bitangent sign (tangent.w) | unchanged |
| bone / armature matrices | `M_world @ axis_basis_change`, where `axis_basis_change = [[1,0,0,0],[0,0,1,0],[0,-1,0,0],[0,0,0,1]]`; IBM = `(axis_basis_change @ arm.matrix_world @ bone.matrix_local)^-1` |
| camera and light nodes | `matrix_world @ Quaternion((√2/2, -√2/2, 0, 0))`, a -90° rotation about local X, so that Blender's -Z view axis stays glTF's -Z after the swap |

More conversion rules:
- **Every object's TRS is converted on its own.** There is no root correction node. Child transforms are computed as `parent.matrix_world⁻¹ @ child.matrix_world`, then decomposed and swizzled.
- Values within `2e-6` of the identity are snapped. A TRS component that equals the identity is **omitted**. **`node.matrix` is never written** **[V-src]**.
- Rotations are normalised before writing.
- A skinned mesh node gets **no TRS**, because the spec ignores it. Its vertex data is baked with `object.matrix_world`, which puts it in world space before the Y-up swap.
- **Unit scale: the exporter ignores `scene.unit_settings.scale_length`.** No scaling is applied. The *importer* does divide by `scale_length`, so a non-1.0 scale does not round-trip **[V-src]**.

### 4.2 Which data becomes a mesh [V-src]
- **MESH** objects export their mesh data. **CURVE, SURFACE and FONT** objects are converted to a mesh. **POINTCLOUD** becomes a POINTS primitive. **Empties** become plain nodes. **ARMATURE** becomes a node plus bone nodes, unless `export_armature_object_remove`. **CAMERA** and **LIGHT** objects become nodes, *and are only exported when `export_cameras` or `export_lights` is on; both default to False* **[V-rna]**.
- **`export_apply=False` (the default):**
  - The **original `obj.data` is used and modifiers are ignored entirely.** A Subdivision Surface modifier on a sphere still exported 1984 vertices; with `export_apply=True` it exported 7936 **[V-run]**.
  - Because no modifiers are evaluated, objects sharing one mesh datablock (linked duplicates) **share one glTF mesh index** **[V-run]**.
  - The Armature modifier is the exception: it is honoured for skinning.
- **`export_apply=True`:**
  - The mesh comes from the evaluated depsgraph through `to_mesh(preserve_all_data_layers=True)`. Armature modifiers are switched off temporarily while exporting skins.
  - The evaluated mesh has no shape keys, so **`export_apply` drops shape keys**, as the UI warns. Geometry Nodes instances are the exception.
  - Objects without modifiers still use their original data and keep instancing.
- `mesh.name` = **mesh datablock name**, for example `"Cube"`. `node.name` = **object name**, for example `"Box"`.

### 4.3 Splitting into primitives and vertices (`primitive_extract.py`) [V-src][V-run]
1. **Triangulation:** `mesh.calc_loop_triangles()` and `loop_triangles` give Blender's own tessellation. To get identical index buffers, frap must reproduce Blender's looptri:
   - quads become `(0,1,2)` and `(0,2,3)`;
   - n-gons use BLI_polyfill (ear clipping) in the face's projected plane, with no beautify step.
   - This tessellation rule is **[?]**: confirm it with a golden test.
2. **One primitive per *used* material slot index**, in ascending `material_index` order. Slots with no faces produce nothing.
   - With `export_materials='NONE'`, everything goes into one primitive with no material.
   - A primitive whose slot is empty gets **no `material` key**.
   - Loose edges become one LINES primitive (`use_mesh_edges`) and loose vertices one POINTS primitive (`use_mesh_vertices`). Both use the first material slot.
   - UDIM textures split a primitive further, per tile.
3. **Vertex splitting ("dots"):**
   - Every loop (face corner) produces a record: `vertex_index` plus all per-corner data. That data is the normal (rounded to **4 decimals**, `ROUNDING_DIGIT=4`, then normalised), every UV layer (as `u, 1-v`), tangent and sign if enabled, colour attributes, per-face and per-corner custom attributes, and morph normals.
   - Within each primitive, records are de-duplicated with an `np.unique` that compares raw bytes (`fast_structured_np_unique`). `-0.0` is folded into `0.0` first.
   - **Each unique record becomes one glTF vertex.** As a result, a vertex is split at every UV seam, sharp edge, flat face or custom-normal discontinuity, and at every colour attribute discontinuity.
   - **Pitfall: vertex order** is the sorted order of the byte view of the record. It is *not* Blender's vertex order. Golden tests must compare geometry **canonically**, for example as a sorted set of triangles of attribute tuples, not index by index.
   - A default cube gives 24 vertices and 36 indices **[V-run]**.
4. **Normals:**
   - Taken from `mesh.corner_normals`, so smooth, flat, sharp edges, smooth-by-angle and custom normals are all reflected.
   - Zero normals become (0, 0, 1) before the axis swap.
   - Written only when `export_normals=True` (the default).
5. **Tangents:**
   - Only with `export_tangents=True` (default False), *and only when the mesh has a UV map*.
   - Computed by `mesh.calc_tangents()` (MikkTSpace) on the **active render** UV map. They are rounded to 4 decimals.
6. **Indices:** `UNSIGNED_SHORT` when the max index is < 65535, otherwise `UNSIGNED_INT`. **UNSIGNED_BYTE is never used.** `mode` is omitted for triangles.
7. **TEXCOORD_n:** every UV layer in `mesh.uv_layers` order, which is not necessarily active-first. Each is a FLOAT VEC2.
   - A material that references a UV map by Attribute node can add extra TEXCOORDs.
   - `textureInfo.texCoord` is resolved per primitive from the UV Map node feeding the texture. With no UV Map node, the active render UV map is used.
8. **POSITION** accessors get min/max. No other accessor does, apart from animation inputs.
9. **Buffer layout:**
   - **No interleaving, ever.** Every accessor gets its own bufferView, with no `byteStride` and a `byteOffset` of 0 on the accessor. Each view is padded to 4 bytes.
   - The BIN content follows gather order. Image bytes are interleaved where the material was first gathered **[V-run]**.
   - `export_shared_accessors=True` shares one set of attribute accessors across all primitives of a mesh, each primitive with its own indices.

### 4.4 Vertex colours (4.2+ semantics, as in 5.2) [V-src][V-run]
The `export_vertex_color` setting decides which colour attributes are exported:

| Value | Behaviour |
|---|---|
| `MATERIAL` (default) | Exports the colour attributes the material node tree **actually uses**. That means a Color Attribute or Vertex Color node feeding Base Color or Alpha, possibly through a Mix MULTIPLY. |
| `ACTIVE` | Exports the active render colour attribute as COLOR_0. |
| `NAME` | Exports the attribute named `export_vertex_color_name` (default `"Color"`). |
| `NONE` | Exports no colours. |

Two more options change the result:
- `export_all_vertex_colors=True` (the default) exports *all* colour attributes. **If the materials use none of them, it inserts a fake white `COLOR_0`** (UNSIGNED_BYTE normalized, VEC4, all 255), so the extra colours (COLOR_1 and up) do not tint the material.
  - In the test, a cube with one unused BYTE_COLOR attribute exported `COLOR_0` (ubyte, white) and `COLOR_1` (ushort, the real data) **[V-run]**.
- `export_active_vertex_color_when_no_material=True` exports the active colour attribute as COLOR_0 when the object has no material.

Encoding:
- `FLOAT_COLOR` → FLOAT.
- **`BYTE_COLOR` → UNSIGNED_SHORT normalized** (values × 65535 + 0.5). It is not UNSIGNED_BYTE.
- Values come from `attr.data[i].color`, which is **linear** for both types.
- VEC3 is used when the alpha channel is not used, VEC4 when it is.
- Point-domain colours are expanded per corner.
- When a material does not use a given colour, its primitive's data for that colour is forced to 1.0.

### 4.5 Materials (Principled BSDF → glTF) [V-src][V-run]
Materials are found by walking back from the **active Material Output's Surface input** (`search_node_tree.py`). Node groups are entered.

**Unlit detection:**
- The material counts as shadeless if Background or Emission feeds the output directly, or if it is the "lightpath is camera ray" mix pattern.
- Such a material exports `KHR_materials_unlit` with baseColor taken from that node.

**Base colour** (`"Base Color"` socket):
- If the socket is unlinked, `baseColorFactor = clamp01(rgb) + [alpha]`. The factor is **omitted when it equals [1,1,1,1]**. The default Principled base colour (0.8, 0.8, 0.8, 1) is written.
- If an Image Texture is linked, the result is `baseColorTexture`. A **Mix (MULTIPLY, Factor = 1) with a constant colour gives `baseColorFactor`**.
- A Color Attribute node multiplied in sets the vertex-colour usage.
- Alpha comes from the Alpha socket. A texture's Alpha output becomes the A channel of the *same* image: base colour RGB and alpha are packed into one PNG.
  - If the alpha comes from an RGB output, it is converted to greyscale with weights 0.2989/0.5870/0.1140.

**Alpha mode is decided only by the node graph.** It does **not** use `material.blend_method` or `surface_render_method`:

| Alpha socket | Result |
|---|---|
| unlinked and == 1 | OPAQUE (`alphaMode` omitted) |
| Math ROUND before Alpha | MASK, cutoff 0.5, so `alphaCutoff` is omitted |
| `1 - (x < c)` (Math LESS_THAN, then SUBTRACT from 1) | MASK, cutoff = c |
| legacy `x > c` | MASK, cutoff = c |
| constant 0 | MASK, cutoff 0.5 |
| any other constant < 1, or any texture or attribute | BLEND |

**`doubleSided`:** written as `true` when `material.use_backface_culling == False`, which is the Blender default, so most materials are double-sided. When culling is on, the key is omitted. It is forced to false when `KHR_materials_volume` is present.

**Metallic and Roughness:**
- An unlinked socket writes its value, **omitted only when it equals 1**. Default Principled therefore gives `metallicFactor: 0, roughnessFactor: 0.5` **[V-run]**.
- A linked texture gives `metallicRoughnessTexture`. The factor is then omitted unless a Math MULTIPLY by a constant is detected.
- **Image packing** (`image.py`):
  - The source channel comes from a **Separate Color** node output (Red, Green or Blue), or from Alpha. When the texture is plugged in directly, the glTF convention is assumed: Roughness = G, Metallic = B, Occlusion = R.
  - The destination channel is fixed: **Roughness → G, Metallic → B, Occlusion → R**. A missing partner channel is filled with white.
  - If the metallic, roughness and occlusion sources are **images of the same size**, they are combined into **one ORM image** shared by `occlusionTexture` and `metallicRoughnessTexture`. Otherwise they are written separately.
  - A combined image is named `img1-img2` from the source names. When every source shares a file path, the file's base name is used instead.
- **Occlusion has no Principled socket.** It comes from the **`glTF Material Output`** node group, an ordinary custom ShaderNodeGroup the add-on creates. Its legacy name is `glTF Settings`.
  - Its inputs are: `Occlusion`, `Thickness`, `Dispersion`, `Iridescence Factor` and `Iridescence Thickness Minimum` (plus related sockets).
  - **frap needs this node group** to round-trip AO, volume thickness, dispersion and iridescence.
  - `occlusionTexture.strength` comes from a Mix (MIX) node blending with white.

**Normal map:**
- Image Texture (Non-Color) → **Normal Map** node → Normal input gives `normalTexture`.
- `scale` = the Normal Map node's `Strength`, omitted when it is 1. The test wrote `scale: 0.7`.
- `Coat Normal` gives `clearcoatNormalTexture`.

**Emission:**
- The source is the `Emission Color` socket. A dedicated Emission node takes priority.
- Factor = colour × `Emission Strength`.
  - If every channel is ≤ 1, it is written as `emissiveFactor`, omitted when it is all zero. Default Principled has strength 0, so nothing is written.
  - If any channel is > 1, `emissiveStrength = max(channel)` goes into **KHR_materials_emissive_strength** and `emissiveFactor` is divided by that strength.
  - Test: colour (1, 0.5, 0.2) at strength 5 gave `emissiveFactor [1, 0.5, 0.2]` and `emissiveStrength 5` **[V-run]**.
- With an emission texture, the factor defaults to [1,1,1].
- If emission comes from an Emission node with no Principled BSDF, `baseColorFactor` becomes [0,0,0,1].

**Textures and samplers:**
- **Image Texture → sampler:**
  - `interpolation` Closest gives NEAREST / NEAREST_MIPMAP_NEAREST. Anything else gives LINEAR (9729) / LINEAR_MIPMAP_LINEAR (9987). Those filters are *always written*.
  - `extension` REPEAT gives no wrap keys. EXTEND or CLIP gives CLAMP_TO_EDGE. MIRROR gives MIRRORED_REPEAT.
  - Mixed wrap modes are detected from a Separate XYZ → Math (PINGPONG or WRAP) → Combine XYZ chain.
  - Identical samplers are de-duplicated.
- **UV Map node → `texCoord`.** **Mapping node → `KHR_texture_transform`:**
  - Allowed `vector_type` values are POINT, TEXTURE (inverted) and VECTOR.
  - Only Z rotation is exported.
  - Conversion: `offset = [loc.x - sy·sin(rot), 1 - loc.y - sy·cos(rot)]`, `rotation = rot`, `scale = [sx, sy]`. Identity components are omitted.
  - Test: Location (0.1, 0.2), Scale 2 gave `offset [0.1, -1.2], scale [2,2]` **[V-run]**.
  - Import uses the inverse: `offset_b = [o.x + sy·sin r, 1 - o.y - sy·cos r]`.

**Images:**
- **Image format** with `export_image_format=AUTO`: JPEG sources stay JPEG, WebP sources stay WebP, everything else becomes PNG.
  - **If the texture feeds Alpha, it is forced to PNG** (or WebP).
  - With `JPEG`, every non-alpha image is JPEG at `export_image_quality` (default 75). Normal and ORM maps became JPEG in the test **[V-run]**.
  - `WEBP` writes WebP.
  - `NONE` writes no images, and factors are read from default values only.
- **Byte passthrough:** if a single unmodified image is used as-is, with no channel packing and a file or packed source of the matching format, the **original file bytes are copied**. The check is on magic bytes. Otherwise the pixels are re-encoded through a temporary Blender image save.
- `image.name` = Blender image name. JSON field order is `bufferView, mimeType, name`.

**Other material extensions:** sockets on the Principled BSDF, plus the glTF Material Output group where noted.

| glTF | Blender source | Export rule |
|---|---|---|
| KHR_materials_transmission | `Transmission Weight` (+ texture, R) | written when ≠ 0 or textured |
| KHR_materials_volume | `glTF Material Output.Thickness` (required) + a **Volume Absorption** node (`Density` gives attenuationDistance = 1/density; `Color` gives attenuationColor) | only if Thickness ≠ 0 or textured; dropped if transmission is absent |
| KHR_materials_ior | `IOR` (unlinked) | **only if transmission, volume or specular is exported**; `ior` omitted when 1.5 |
| KHR_materials_specular | `Specular IOR Level` × 2 gives specularFactor (Blender's default 0.5 gives 1.0, so it is omitted); `Specular Tint` gives specularColorFactor (if the level × 2 is > 1, the excess is folded into the colour) | `export_original_specular` controls a legacy path |
| KHR_materials_clearcoat | `Coat Weight` (R), `Coat Roughness` (G; Blender default 0.03), `Coat Normal` | written when weight ≠ 0 or textured |
| KHR_materials_sheen | `Sheen Weight` ≠ 0 enables it; `Sheen Tint` gives sheenColorFactor; `Sheen Roughness` gives sheenRoughnessFactor | **the weight is not exported as a scalar**: the colour is just the tint |
| KHR_materials_anisotropy | `Anisotropic` gives strength; `Anisotropic Rotation` × 2π gives rotation (radians); a `Tangent` from a specific node pattern gives the texture | |
| KHR_materials_iridescence | `Thin Film Thickness` gives thicknessMaximum (default 400 nm); `Thin Film IOR` gives iridescenceIor (1.33 matches the glTF default 1.3 closely enough to be omitted); glTF Material Output `Iridescence Factor` / `Iridescence Thickness Minimum` | |
| KHR_materials_dispersion | glTF Material Output `Dispersion` | only together with volume |
| KHR_materials_emissive_strength | see Emission | |
| KHR_materials_unlit | Background or Emission shader directly on the output | |
| KHR_materials_variants | Scene-level `gltf2_KHR_materials_variants_variants` plus per-mesh variant mappings | needs add-on preference `KHR_materials_variants_ui` |

- `export_materials`:
  - `EXPORT` is the default.
  - `PLACEHOLDER` writes an empty material that keeps the primitive split.
  - `VIEWPORT` uses `diffuse_color`, `metallic` and `roughness` from the material's viewport display.
  - `NONE` merges everything into one primitive.
- Blender 5.2 RNA notes **[V-rna]**:
  - `Material.blend_method` still exists but is legacy.
  - `surface_render_method ∈ {DITHERED, BLENDED}`. The *importer* sets BLENDED for BLEND and DITHERED otherwise.
  - `Material.use_nodes` is deprecated, to be removed in 6.0, because materials always have nodes.
  - The 5.2 Principled BSDF also has `Thin Wall`, `Weight` and `Diffuse Roughness` inputs, which the glTF exporter ignores.

### 4.6 Shape keys → morph targets [V-src][V-run]
- Shape keys are exported when `export_morph=True`, the mesh has more than one key block, and either `export_apply` is off or no modifiers were applied.
- **These key blocks are skipped:** the first one (Basis), muted keys, and keys whose `relative_key` is themselves.
- **Base positions come from `key_blocks[0].relative_key`**, not the evaluated mix. Deltas = `key.co - basis.co`, with NaN replaced by 0.
- Normal deltas use `key.normals_split_get()` minus the basis split normals (`export_morph_normal`, default True). Tangent deltas are off by default (`export_morph_tangent`).
- `mesh.weights = [key.value ...]` and `mesh.extras.targetNames = [key.name ...]`.
- **Sparse accessors** (`export_try_sparse_sk`, default True):
  - Used when `indices + values + 170 + 64 bytes < dense size`, with the smallest index type that fits.
  - The sparse accessor has **no bufferView**, so its base is zeros.
  - An all-zero target still gets a sparse accessor with one index, `[0]`. With `export_try_omit_sparse_sk` it is omitted entirely: no bufferView and no sparse, which means zeros.
  - Test: moving one vertex of a UV sphere produced a sparse accessor with count 32 and UNSIGNED_BYTE indices, because that vertex was split into 32 corners **[V-run]**.

### 4.7 Skinning [V-src][V-run]
- Skinning is exported when the object has an **Armature modifier** with an object, has vertex groups, at least one vertex is weighted, and the object is not bone-parented to that same armature.
- **Weights:**
  - Weights ≤ 1e-4 are dropped, and groups with no matching bone are ignored.
  - Each vertex's influences are sorted by weight, descending, and capped at `export_influence_nb` (default 4) unless `export_all_influences`. That gives sets of 4: JOINTS_n / WEIGHTS_n.
  - Weights are **renormalised to sum 1** and stored as FLOAT VEC4.
  - JOINTS are **UNSIGNED_BYTE if every joint < 256**, otherwise UNSIGNED_SHORT. Unused slots are joint 0 with weight 0.
- A vertex with no influence is bound to an extra **`neutral_bone`** joint at the armature root.
- **Joint order:** depth-first over the bones. `skin.name` = armature object name, and `skeleton` is not written.
- **IBM:** `inverse(axis_basis_change @ armature.matrix_world @ bone.matrix_local)` per joint, column-major MAT4 in a bufferView with no target.
- Bones become nodes.
  - With `export_rest_position_armature` (the default), they use the **rest pose** (`bone.matrix_local`). Otherwise the current pose is used.
  - The bone's world matrix is post-multiplied by `axis_basis_change` before the local TRS is extracted. The joint node's local axes are therefore Blender's bone axes (the bone points along local +Y) expressed in Y-up space. No per-bone re-orientation is done on export.
- The armature object becomes a parent node. Test: `Rig` translation (3,0,0) was the parent of the `Root` bone and the `Skinned` mesh node **[V-run]**.
- Skinned vertex data is transformed by the mesh `matrix_world` (positions), the inverse-transpose of the relevant matrix (normals) and the rotation part (tangents).
- Options:
  - `export_def_bones` keeps only deform bones, plus bones that have object children.
  - `export_leaf_bone` adds tip bones.
  - `export_hierarchy_flatten_bones` and `export_hierarchy_flatten_objs` flatten hierarchies with non-decomposable matrices.
  - `export_armature_object_remove` drops the armature node when there is a single root bone.

### 4.8 Animation [V-src][V-run]
Blender 5.x actions are **layered and slotted**: `Action.slots`, `Action.layers[].strips[].channelbag(slot).fcurves`, and there is **no `Action.fcurves`** **[V-rna]**.
- One action can drive several IDs. In the test, action `MoveAction` had slots `OBMover` and `KEKey`, so it drove the object and its shape-key datablock at once **[V-run]**.
- frap's animation data model should copy the slot concept.

**`export_animation_mode`:**

| Mode | Result |
|---|---|
| `ACTIONS` (default) | One glTF animation **per action**, named after the action. It includes the active action and actions on NLA tracks, and channels from different objects that use the same action are merged (`export_merge_animation='ACTION'`). In the test, `MoveAction` produced translation, rotation and weights channels in one animation **[V-run]**. |
| `ACTIVE_ACTIONS` | All currently assigned actions merged into one animation named `export_nla_strips_merged_animation_name` (default `"Animation"`). |
| `BROADCAST` | Every compatible action goes to every object. |
| `NLA_TRACKS` | One animation per NLA track name. Sampling is always on. **An object with no NLA tracks exports no animation** **[V-run]**. |
| `SCENE` | Bakes the scene frame range. With `export_anim_scene_split_object` (default True), there is one animation per object, named after the object. In the test, frames 1–20 gave 20 keys **[V-run]**. |

**Sampling** (`export_force_sampling=True`, the default):
- Every channel is evaluated through the depsgraph every `export_frame_step` frames, from the action's `frame_range`. With `export_frame_range`, the scene range is used instead.
- Output uses `LINEAR`. For bones and objects, all three paths (T, R, S) are written per animated bone.
- `export_optimize_animation_size` collapses constant channels to **2 keys with STEP**. `export_optimize_animation_keep_anim_armature` keeps at least that for every bone **[V-run]**.

**No sampling** (`export_force_sampling=False`):
- F-curves are exported directly: BEZIER → **CUBICSPLINE**, LINEAR → LINEAR, CONSTANT → STEP.
- Tangents are computed from the Bézier handles: `slope = (handle.y - co.y)/(handle.x - co.x)` per frame, × fps, giving units per second.
- The first in-tangent and the last out-tangent are zero.
- Test: 2 keys gave 2 inputs and 6 outputs **[V-run]**.
- **The exporter falls back to sampling** when interpolations are mixed, keys are not aligned across components, an interpolation is unsupported (elastic and the like), there is a single keyframe, constraints are present, or the rotation mode is not convertible.
- Euler rotation is always converted to a quaternion.

**Time:**
- `seconds = frame / (render.fps * render.fps_base)`. **This is a Blender bug for fps_base ≠ 1**: the correct formula is frame × fps_base / fps. frap should copy it in "blender-compat" mode and flag it.
- Times are not shifted unless `export_anim_slide_to_zero` is set, so frame 1 at 24 fps gives t = 0.041666 **[V-run]**. Negative frames are slid (`export_negative_frame=SLIDE`) or cropped.
- Animation sampler input accessors carry min/max **[V-run]**.

**Shape-key value animation** becomes `path: weights`. One channel covers all targets, and targets that are not animated are filled with their current values.

**`export_bake_animation`** samples every object, including ones without actions, such as constraint- or driver-driven objects.

**Animation pointer** (material, light and camera properties) works only in NLA_TRACKS or SCENE mode with `export_pointer_animation`.

### 4.9 Cameras [V-src][V-run]
- Perspective:
  - `aspectRatio = (res_x·pixel_aspect_x)/(res_y·pixel_aspect_y)`, always written.
  - `yfov = 2·atan(tan(angle/2)/aspect)` if width ≥ height and `sensor_fit ≠ VERTICAL`. If width ≥ height with VERTICAL, `yfov = angle`. If width < height: `yfov = angle` unless `sensor_fit == HORIZONTAL`, in which case the atan formula is used.
  - `znear = clip_start`, `zfar = clip_end`.
  - Test: the default camera (50 mm lens, 36 mm sensor, 1920×1080) gave `yfov 0.3996, aspectRatio 1.7778, zfar 100, znear 0.1` **[V-run]**.
- Orthographic: `xmag = ortho_scale·(sx/max(sx,sy))/2`, and `ymag` likewise.
- `camera.name` = camera datablock name. Panoramic cameras are not exported.
- Import sets `angle_y = yfov`, `sensor_fit = 'VERTICAL'` and `lens_unit = 'FOV'`. A missing `zfar` becomes 1e12. Ortho uses `ortho_scale = 2·max(xmag, ymag)`. The render resolution is not changed.

### 4.10 Lights [V-src][V-run]
- Blender types map as follows: POINT → point, SUN → directional, SPOT → spot. **AREA (and HEMI) are not exported**, with a warning.
- `color = light.color × temperature_color` (if `use_temperature`) × the colour of the Emission node in the light's node tree. The node tree is ignored when the render engine is `BLENDER_EEVEE`.
- Base power P: `light.energy`, times `light.area(matrix_world)` if `light.normalize` is False. Then **`export_import_convert_lighting_mode`** applies:
  - **SPEC** (default, physically based): point and spot `cd = P/(4π)·683`; sun `lux = P·683`. Here P for the sun is in W/m².
  - **COMPAT** (unitless): point and spot `P/(4π)`; sun `P`.
  - **RAW** (deprecated): `P`.
  - Every mode then multiplies by `2^light.exposure`.
  - Test: the default 1000 W point light gave `intensity 54351.41` (= 1000/(4π)·683) **[V-run]**.
- Import is the inverse. SPEC point: `W = cd·4π/683`. SPEC sun: `W/m² = lux/683`. COMPAT: ×4π (point) or 1 (sun).
- Spot: `outerConeAngle = spot_size/2` and `innerConeAngle = outer - outer·spot_blend`. Import uses `spot_size = 2·outer` and `spot_blend = 1 - inner/outer`.
- `range` is written **only if `use_custom_distance`**, with the value `cutoff_distance`. **Import ignores `range`** (marked TODO in the source).
- `light.name` = light datablock name. The node gets the -90° X correction (§4.1).

### 4.11 Naming [V-src]
- Node name = object name, including dotted suffixes such as `"Cube.001"`, written verbatim **[V-run]**. Bone nodes use the bone name.
- Mesh, material, camera and light names = datablock names. Image name = Blender image name.
- Scene name = scene name (`"Scene"`). Animation name = action name, NLA track name or object name, depending on mode.
- Names are **not** made unique on export.
- **Import** names datablocks through `find_unused_name`: if the name is taken, `.001`, `.002` and so on are appended, capped at 63 UTF-8 bytes like Blender's name limit.
  - Unnamed nodes become `"Node_<i>"`, unnamed meshes `"Mesh_<i>"`, unnamed lights `"Sun"`, `"Point"` or `"Spot"`, unnamed cameras `"Camera"`.

### 4.12 Other export-side behaviours worth mirroring
- Collections:
  - `export_hierarchy_full_collections` emits collection nodes.
  - Collection instances are exported as nodes with instance children, with `instance_offset` applied.
  - `at_collection_center` re-centres the export on the collection.
- `use_selection`, `use_visible`, `use_renderable`, `use_active_collection` (+`use_active_collection_with_nested`), `use_active_scene` and `collection` filter what is exported. Objects in a collection named `glTF_not_exported` are always skipped.
- Multiple scenes are exported as multiple glTF scenes unless `use_active_scene`. `scene` = the active scene index.
- `export_gn_mesh` exports Geometry Nodes instances, which is experimental.
- `will_save_settings` stores the operator settings in `scene["glTF2ExportSettings"]`.

## 5. glTF → Blender import (`import_scene.gltf`) [V-src]

- **Coordinates:** location `(x,y,z) → u·(x, -z, y)`; quaternion xyzw → `(w, x, -z, y)`; scale `(x,y,z) → (x,z,y)`. Here `u = 1/scene.unit_settings.scale_length`. Cameras and lights get the correction `Quaternion((√2/2, √2/2, 0, 0))`.
- **Meshes:**
  - All primitives of one glTF mesh become **one Blender mesh**, with a material slot per primitive material. `import_merge_material_slots` (default True) merges duplicate slots.
  - Vertices are **not** merged unless `merge_vertices=True`, which merges co-located vertices with equal attributes, except those whose normals differ.
  - Faces come from TRIANGLES, plus strips and fans converted to triangles. LINES and POINTS become loose edges and vertices. With `import_point_as_pointcloud`, a POINTS-only mesh becomes a PointCloud object.
  - UVs: `v → 1-v`. COLOR_n becomes colour attributes. Morph targets become shape keys, named from `extras.targetNames`, otherwise the POSITION accessor name, otherwise `"target_<i>"`, with `mesh.weights` as their values.
  - Custom `_` attributes become mesh attributes.
- **`import_shading`:**
  - `NORMALS` (default): sets custom split normals from NORMAL. A face is marked flat (`sharp_face`) when all three corner normals equal the face normal (dot > 0.9999999).
  - `FLAT`: all faces flat.
  - `SMOOTH`: all faces smooth, except primitives that have no NORMAL.
- **Materials:**
  - The importer rebuilds a Principled BSDF graph: Image Texture, Separate Color (G → Roughness, B → Metallic), Normal Map, Mapping, UV Map, Mix multiply for factors, Color Attribute, Math nodes for alpha clip, and Volume Absorption.
  - AO, thickness, dispersion and iridescence go into a `glTF Material Output` group node.
  - `alphaMode` BLEND gives `surface_render_method = 'BLENDED'`. OPAQUE and MASK give `DITHERED`. `doubleSided` false gives `use_backface_culling = True`.
- **Images:** packed into the .blend when `import_pack_images` (default True). `import_webp_texture` prefers the WebP source over the fallback.
- **Armatures:**
  - Nodes used as joints become bones of a single armature, placed at the deepest common ancestor.
  - Skinned meshes are moved under it, and vertices are re-skinned into the chosen bind pose.
  - `guess_original_bind_pose` (default True) derives each bone's edit-mode rest from the IBMs: `bind_local = IBM(parent) · IBM(joint)⁻¹`. When off, the node's default TRS is used.
  - `bone_heuristic`:
    - `BLENDER` (default): every bone is rotated by a fixed +90° about X, which is the best for round-trips.
    - `TEMPERANCE`: points each bone at the centroid of its children, snapped to the axes.
    - `FORTUNE`: like TEMPERANCE but without snapping.
  - Bone shapes are created unless `disable_bone_shape` is set; `bone_shape_scale_factor` scales them.
- **Animation:** each glTF animation becomes an action (in 5.x, actions with slots). The importer pushes them onto NLA tracks named after the animation (`simulate_stash`), and the first animation stays active [?]. KHR_animation_pointer is imported.
- **Extensions handled on import** **[V-src]**:
  - Material and texture: KHR_materials_pbrSpecularGlossiness, unlit, clearcoat, emissive_strength, transmission, specular, sheen, ior, volume, anisotropy, dispersion, iridescence, variants; KHR_texture_transform; EXT_texture_webp.
  - Lights and animation: KHR_lights_punctual, KHR_animation_pointer.
  - Compression and geometry: KHR_mesh_quantization, KHR_draco_mesh_compression, EXT_meshopt_compression, KHR_meshopt_compression, EXT_mesh_gpu_instancing.
- **Not handled on import:** KHR_texture_basisu, KHR_node_visibility, KHR_materials_diffuse_transmission, KHR_xmp_json_ld, KHR_interactivity, KHR_gaussian_splatting.
  - If an unsupported extension appears in `extensionsRequired`, the import fails with an error.
- Other import options: `import_scene_extras` copies scene extras into custom properties; `import_scene_as_collection` creates a collection per glTF scene; `import_unused_materials` imports materials and images that no primitive uses; `import_select_created_objects` selects the new objects.

## 6. glTF features that the Blender 5.2 exporter does NOT produce
These matter because frap must still be able to *create and edit* them, while staying Blender-identical by default:
- interleaved bufferViews (byteStride), `node.matrix`, `skin.skeleton`, quantised attributes (except through gltfpack), UNSIGNED_BYTE indices;
- LINE_LOOP, LINE_STRIP, TRIANGLE_STRIP and TRIANGLE_FAN modes;
- TEXCOORD or COLOR morph targets;
- KHR_texture_basisu, KHR_node_visibility, KHR_materials_diffuse_transmission;
- area lights, which have no glTF equivalent;
- multiple UV sets per texture beyond what the node graph expresses;
- `range` on import.
## 7. Implications and recommended decisions for frap

1. **Keep one internal scene model and two glTF writers.**
   - **Blender-compat mode** (the default for `export_scene.gltf`) reproduces every rule in §4: non-interleaved views, gather order, omission rules, frame 0 rest, the fake COLOR_0, BYTE_COLOR → USHORT, ushort/uint indices, sparse heuristic, `extensionsRequired` for lights and texture_transform, and so on.
   - A **spec-complete layer** produces what Blender cannot: quantisation, interleaving, strips, `node.matrix`, basisu, node_visibility and diffuse_transmission. It is enabled by frap-namespaced operator properties (for example `frap_*`), so `export_scene.gltf` keeps exactly Blender's 110 property names.
   - Reading must accept *everything* in §2 and §3.
2. **Do not try to make the GLB byte-identical.**
   - Vertex order (byte-sorted `np.unique`), float repr, node post-order and image re-encoding make byte equality fragile.
   - Golden tests should parse both GLBs and compare:
     - the JSON structure, with array order normalised by name;
     - accessor *decoded* data with a tolerance (about 1e-5; normals have already been rounded to 1e-4);
     - geometry as a **canonical multiset of triangles**, where each corner is a tuple of all its attributes;
     - images by decoded pixels;
     - `asset.generator` is ignored.
3. **Vertex splitting key:** split per corner on (vertex index, normal rounded to 4 dp then normalised, every UV layer, colour attributes, tangent and sign, morph normals). Then de-duplicate within each material primitive. This reproduces Blender's counts exactly: the default cube gives 24 vertices.
4. **Triangulation must match Blender's looptris.** Otherwise index counts can match while the triangles differ. Verify quads and n-gons with a golden test **[?]**.
5. **Material model:**
   - frap's node tree only has to *recognise* the patterns in §4.5.
   - It must include a **`glTF Material Output` group node** with sockets Occlusion, Thickness, Dispersion, Iridescence Factor and Iridescence Thickness Minimum, because Principled BSDF has no AO socket.
   - It must include a **Volume Absorption** node for KHR_materials_volume.
   - Pack images as Blender does: Separate Color channel selection; destination channels R = AO, G = roughness, B = metallic; base colour RGB + Alpha in one PNG; RGB → greyscale weights 0.2989/0.5870/0.1140.
6. **The image pipeline needs** PNG, JPEG (quality 75) and WebP encode/decode in Rust (`image` crate + `webp`). Copy source bytes through whenever no repacking is needed, as Blender does. KTX2/BasisU is optional and needs FFI.
7. **Compression:**
   - **Draco** has no mature pure-Rust *encoder*. The options are FFI to Google's draco C++ library, or shipping it as an optional feature.
   - **meshopt** has the `meshopt` crate (FFI to meshoptimizer) for encoding and decoding.
   - Both are export-off by default in Blender. The importer must decode both.
   - `export_use_gltfpack` calls an external `gltfpack` binary. Accept the property and return a clear JSON error if the binary is missing.
8. **Animation model:**
   - Mirror Blender 5.x slotted layered actions: an Action with slots, layers, strips and channelbags holding F-curves, with `keyframe_insert` and NLA tracks and strips.
   - The exporter needs a **depsgraph-like evaluator** (F-curve evaluation with Bézier handles, extrapolation, NLA blending, constraints if they are ever supported) because sampling is the default.
   - Copy the fps_base time bug only in compat mode, and flag it.
9. **Lights:** store Blender units (W, W/m², spot_size, spot_blend, exposure, temperature, normalize, use_custom_distance) and convert at I/O using §4.10. AREA lights cannot be exported, so warn just as Blender does.
10. **Cameras:** store Blender's camera (lens or angle, sensor_fit, sensor width and height, shift, clip). Use render resolution and pixel aspect at export, as in §4.9.
11. **Unit scale:** follow Blender: no scaling on export, and divide by `scale_length` on import. Document the asymmetry.
12. **Defaults that surprise users and LLM agents.** Surface these in `--describe`:
    - `export_cameras=False`, `export_lights=False`, `export_extras=False`, `export_apply=False` (modifiers are ignored!), `export_tangents=False`;
    - rest frame 0;
    - an unused colour attribute adds a fake white COLOR_0.
13. **Import fidelity:** reuse the same mapping in reverse.
    - Shading-from-normals detection, `bone_heuristic` (BLENDER = +90° X roll), and `guess_original_bind_pose` from the IBMs.
    - Import has to rebuild a node tree that the frap exporter maps back losslessly. The test is that a GLB → frap → GLB round-trip is identical modulo vertex order.
14. **KHR_node_visibility:**
    - Blender 5.2 ignores it, so exporting it by default would break parity.
    - Import it into `hide_viewport` and `hide_render` [?], and export it only behind a frap-namespaced flag.
    - The same reasoning applies to KHR_materials_diffuse_transmission (RC), which has no Principled mapping in 5.2 [?].

## 8. Uncertainties and open questions
- [?] Exact Blender looptri rule for quads and n-gons: whether the quad split is always (0,2) or chooses a diagonal. Verify with a golden test on a non-planar quad.
- [?] Whether KHR_meshopt_compression is ratified, and its exact differences from EXT (v1 codecs, COLOR filter).
- [?] The output component type for animating KHR_node_visibility through KHR_animation_pointer.
- [?] Import-side NLA layout details (which animation stays active) and the exact interaction with slots. Verify with a multi-animation import.
- [?] The full unlit-detection patterns and the full anisotropy and iridescence texture node patterns. These are summarised here, not fully transcribed; read `search_node_tree.py` (`detect_anisotropy_nodes`, `detect_iridescence_thickness_texure`) and `unlit.py` when implementing.
- [?] How `export_vertex_color` and the fake COLOR_0 interact when *some* materials use colours and others do not. The code forces unused colours to 1.0 per material. Verify with a two-material test.
- Several behaviours are Blender quirks rather than spec requirements: fps_base, tuple floats, KHR_texture_transform marked required, KHR_lights_punctual marked required, light `range` ignored on import. frap should copy them in compat mode and document each one.

---

## 9. Exhaustive operator property lists (Blender 5.2.4, add-on 5.2.41) [V-rna]

`export_format` uses a **dynamic enum**. In background mode the RNA dump returns no items. From the source **[V-src]**, the items are `GLB` (index 0, the default), `GLTF_SEPARATE`, and `GLTF_EMBEDDED`, which is only offered when the add-on preference `allow_embedded_format` is on.

The draco and meshopt properties are honoured only when the bundled bridge libraries `libbf_intern_draco_bridge` and `libbf_intern_meshopt_bridge` load. Both are present in this build. Without them the exporter silently disables compression.

### 9.1 `bpy.ops.export_scene.gltf` (110 properties)

| Property | Type | Default | Description |
|---|---|---|---|
| `filepath` | STRING | `""` | Filepath used for exporting the file |
| `check_existing` | BOOLEAN | `True` | Check and warn on overwriting existing files (hidden) |
| `export_import_convert_lighting_mode` | ENUM: `SPEC` / `COMPAT` / `RAW` | `'SPEC'` | Optional backwards compatibility for non-standard render engines. Applies to lights |
| `gltf_export_id` | STRING | `""` | Identifier of caller (in case of add-on calling this exporter). Can be useful in case of Extension added by other add-ons |
| `export_use_gltfpack` | BOOLEAN | `False` | Use gltfpack to simplify the mesh and/or compress its textures |
| `export_gltfpack_tc` | BOOLEAN | `True` | Convert all textures to KTX2 with BasisU supercompression |
| `export_gltfpack_tq` | INT [1..10] | `8` | Texture encoding quality |
| `export_gltfpack_si` | FLOAT [0.0..1.0] | `1.0` | Simplify meshes targeting triangle count ratio |
| `export_gltfpack_sa` | BOOLEAN | `False` | Aggressively simplify to the target ratio disregarding quality |
| `export_gltfpack_slb` | BOOLEAN | `False` | Lock border vertices during simplification to avoid gaps on connected meshes |
| `export_gltfpack_vp` | INT [1..16] | `14` | Use N-bit quantization for positions |
| `export_gltfpack_vt` | INT [1..16] | `12` | Use N-bit quantization for texture coordinates |
| `export_gltfpack_vn` | INT [1..16] | `8` | Use N-bit quantization for normals and tangents |
| `export_gltfpack_vc` | INT [1..16] | `8` | Use N-bit quantization for colors |
| `export_gltfpack_vpi` | ENUM: `Integer` / `Normalized` / `Floating-point` | `'Integer'` | Type to use for vertex position attributes |
| `export_gltfpack_noq` | BOOLEAN | `True` | Disable quantization; produces much larger glTF files with no extensions |
| `export_gltfpack_kn` | BOOLEAN | `False` | Restrict some optimization to keep named nodes and meshes attached to named nodes so that named nodes can be transformed externally |
| `export_format` | ENUM: `GLB` / `GLTF_SEPARATE` (+ `GLTF_EMBEDDED` if add-on pref allow_embedded_format) — dynamic items | `'GLB' (index 0)` | Output format. Binary is most efficient, but JSON may be easier to edit later |
| `ui_tab` | ENUM: `GENERAL` / `MESHES` / `OBJECTS` / `ANIMATION` | `'GENERAL'` | Export setting categories |
| `export_copyright` | STRING | `""` | Legal rights and conditions for the model |
| `export_image_format` | ENUM: `AUTO` / `JPEG` / `WEBP` / `NONE` | `'AUTO'` | Output format for images. PNG is lossless and generally preferred, but JPEG might be preferable for web applications due to the smaller file size. Alternatively they can be omitted if they are not needed |
| `export_image_add_webp` | BOOLEAN | `False` | Creates WebP textures for every texture. For already WebP textures, nothing happens |
| `export_image_webp_fallback` | BOOLEAN | `False` | For all WebP textures, create a PNG fallback texture |
| `export_texture_dir` | STRING | `""` | Folder to place texture files in. Relative to the .gltf file |
| `export_jpeg_quality` | INT [0..100] | `75` | Quality of JPEG export |
| `export_image_quality` | INT [0..100] | `75` | Quality of image export |
| `export_keep_originals` | BOOLEAN | `False` | Keep original textures files if possible. WARNING: if you use more than one texture, where pbr standard requires only one, only one texture will be used. This can lead to unexpected results |
| `export_texcoords` | BOOLEAN | `True` | Export UVs (texture coordinates) with meshes |
| `export_normals` | BOOLEAN | `True` | Export vertex normals with meshes |
| `export_gn_mesh` | BOOLEAN | `False` | Export Geometry nodes instance meshes |
| `export_meshopt_compression_enable` | BOOLEAN | `False` | Compress mesh using Meshopt |
| `export_meshopt_extension` | ENUM: `EXT_meshopt_compression` / `KHR_meshopt_compression` | `'EXT_meshopt_compression'` | Extension to use for meshopt compression |
| `export_draco_mesh_compression_enable` | BOOLEAN | `False` | Compress mesh using Draco |
| `export_draco_mesh_compression_level` | INT [0..10] | `6` | Compression level (0 = most speed, 6 = most compression, higher values currently not supported) |
| `export_draco_position_quantization` | INT [0..30] | `14` | Quantization bits for position values (0 = no quantization) |
| `export_draco_normal_quantization` | INT [0..30] | `10` | Quantization bits for normal values (0 = no quantization) |
| `export_draco_texcoord_quantization` | INT [0..30] | `12` | Quantization bits for texture coordinate values (0 = no quantization) |
| `export_draco_color_quantization` | INT [0..30] | `10` | Quantization bits for color values (0 = no quantization) |
| `export_draco_generic_quantization` | INT [0..30] | `12` | Quantization bits for generic values like weights or joints (0 = no quantization) |
| `export_tangents` | BOOLEAN | `False` | Export vertex tangents with meshes |
| `export_materials` | ENUM: `EXPORT` / `PLACEHOLDER` / `VIEWPORT` / `NONE` | `'EXPORT'` | Export materials |
| `export_unused_images` | BOOLEAN | `False` | Export images not assigned to any material |
| `export_unused_textures` | BOOLEAN | `False` | Export image texture nodes not assigned to any material. This feature is not standard and needs an external extension to be included in the glTF file |
| `export_vertex_color` | ENUM: `MATERIAL` / `ACTIVE` / `NAME` / `NONE` | `'MATERIAL'` | How to export vertex color |
| `export_vertex_color_name` | STRING | `Color` | Name of vertex color to export |
| `export_all_vertex_colors` | BOOLEAN | `True` | Export all vertex colors, even if not used by any material. If no Vertex Color is used in the mesh materials, a fake COLOR_0 will be created, in order to keep material unchanged |
| `export_active_vertex_color_when_no_material` | BOOLEAN | `True` | When there is no material on object, export active vertex color |
| `export_attributes` | BOOLEAN | `False` | Export Attributes (when starting with underscore) |
| `use_mesh_edges` | BOOLEAN | `False` | Export loose edges as lines, using the material from the first material slot |
| `use_mesh_vertices` | BOOLEAN | `False` | Export loose points as glTF points, using the material from the first material slot |
| `export_cameras` | BOOLEAN | `False` | Export cameras |
| `use_selection` | BOOLEAN | `False` | Export selected objects only |
| `use_visible` | BOOLEAN | `False` | Export visible objects only |
| `use_renderable` | BOOLEAN | `False` | Export renderable objects only |
| `use_active_collection_with_nested` | BOOLEAN | `True` | Include active collection and nested collections |
| `use_active_collection` | BOOLEAN | `False` | Export objects in the active collection only |
| `use_active_scene` | BOOLEAN | `False` | Export active scene only |
| `collection` | STRING | `""` | Export only objects from this collection (and its children) |
| `at_collection_center` | BOOLEAN | `False` | Export at Collection center of mass of root objects of the collection |
| `export_extras` | BOOLEAN | `False` | Export custom properties as glTF extras |
| `export_yup` | BOOLEAN | `True` | Export using glTF convention, +Y up |
| `export_apply` | BOOLEAN | `False` | Apply modifiers (excluding Armatures) to mesh objects -WARNING: prevents exporting shape keys |
| `export_shared_accessors` | BOOLEAN | `False` | Export Primitives using shared accessors for attributes |
| `export_animations` | BOOLEAN | `True` | Exports active actions and NLA tracks as glTF animations |
| `export_frame_range` | BOOLEAN | `False` | Clips animations to selected playback range |
| `export_frame_step` | INT [1..120] | `1` | How often to evaluate animated values (in frames) |
| `export_force_sampling` | BOOLEAN | `True` | Apply sampling to all animations |
| `export_sampling_interpolation_fallback` | ENUM: `LINEAR` / `STEP` | `'LINEAR'` | Interpolation fallback for sampled animations, when the property is not keyed |
| `export_pointer_animation` | BOOLEAN | `False` | Export material, Light & Camera animation as Animation Pointer. Available only for baked animation mode 'NLA Tracks' and 'Scene' |
| `export_animation_mode` | ENUM: `ACTIONS` / `ACTIVE_ACTIONS` / `BROADCAST` / `NLA_TRACKS` / `SCENE` | `'ACTIONS'` | Export Animation mode |
| `export_nla_strips_merged_animation_name` | STRING | `Animation` | Name of single glTF animation to be exported |
| `export_def_bones` | BOOLEAN | `False` | Export Deformation bones only |
| `export_hierarchy_flatten_bones` | BOOLEAN | `False` | Flatten Bone Hierarchy. Useful in case of non decomposable transformation matrix |
| `export_hierarchy_flatten_objs` | BOOLEAN | `False` | Flatten Object Hierarchy. Useful in case of non decomposable transformation matrix |
| `export_armature_object_remove` | BOOLEAN | `False` | Remove Armature object if possible. If Armature has multiple root bones, object will not be removed |
| `export_leaf_bone` | BOOLEAN | `False` | Append a final bone to the end of each chain to specify last bone length (use this when you intend to edit the armature from exported data) |
| `export_optimize_animation_size` | BOOLEAN | `True` | Reduce exported file size by removing duplicate keyframes |
| `export_optimize_animation_keep_anim_armature` | BOOLEAN | `True` | If all keyframes are identical in a rig, force keeping the minimal animation. When off, all possible channels for the bones will be exported, even if empty (minimal animation, 2 keyframes) |
| `export_optimize_animation_keep_anim_object` | BOOLEAN | `False` | If all keyframes are identical for object transformations, force keeping the minimal animation |
| `export_optimize_disable_viewport` | BOOLEAN | `False` | When exporting animations, disable viewport for other objects, for performance |
| `export_negative_frame` | ENUM: `SLIDE` / `CROP` | `'SLIDE'` | Negative Frames are slid or cropped |
| `export_anim_slide_to_zero` | BOOLEAN | `False` | Set all glTF animation starting at 0.0s. Can be useful for looping animations |
| `export_bake_animation` | BOOLEAN | `False` | Force exporting animation on every object. Can be useful when using constraints or driver. Also useful when exporting only selection |
| `export_merge_animation` | ENUM: `NLA_TRACK` / `ACTION` / `NONE` | `'ACTION'` | Merge Animations |
| `export_anim_single_armature` | BOOLEAN | `True` | Export all actions, bound to a single armature. WARNING: Option does not support exports including multiple armatures |
| `export_reset_pose_bones` | BOOLEAN | `True` | Reset pose bones between each action exported. This is needed when some bones are not keyed on some animations |
| `export_current_frame` | BOOLEAN | `False` | Export the scene in the current animation frame. When off, frame 0 is used as rest transformations for objects |
| `export_rest_position_armature` | BOOLEAN | `True` | Export armatures using rest position as joints' rest pose. When off, current frame pose is used as rest pose |
| `export_anim_scene_split_object` | BOOLEAN | `True` | Export Scene as seen in Viewport, But split animation by Object |
| `export_skins` | BOOLEAN | `True` | Export skinning (armature) data |
| `export_influence_nb` | INT [1..2147483647] | `4` | Choose how many Bone influences to export |
| `export_all_influences` | BOOLEAN | `False` | Allow export of all joint vertex influences. Models may appear incorrectly in many viewers |
| `export_morph` | BOOLEAN | `True` | Export shape keys (morph targets) |
| `export_morph_normal` | BOOLEAN | `True` | Export vertex normals with shape keys (morph targets) |
| `export_morph_tangent` | BOOLEAN | `False` | Export vertex tangents with shape keys (morph targets) |
| `export_morph_animation` | BOOLEAN | `True` | Export shape keys animations (morph targets) |
| `export_morph_reset_sk_data` | BOOLEAN | `True` | Reset shape keys between each action exported. This is needed when some SK channels are not keyed on some animations |
| `export_lights` | BOOLEAN | `False` | Export directional, point, and spot lights. Uses "KHR_lights_punctual" glTF extension |
| `export_try_sparse_sk` | BOOLEAN | `True` | Try using Sparse Accessor if it saves space |
| `export_try_omit_sparse_sk` | BOOLEAN | `False` | Omitting Sparse Accessor if data is empty |
| `export_gpu_instances` | BOOLEAN | `False` | Export using EXT_mesh_gpu_instancing. Limited to children of a given Empty. Multiple materials might be omitted |
| `export_action_filter` | BOOLEAN | `False` | Filter Actions to be exported |
| `export_convert_animation_pointer` | BOOLEAN | `False` | Export TRS and weights as Animation Pointer. Using KHR_animation_pointer extension |
| `export_nla_strips` | BOOLEAN | `True` | When on, multiple actions become part of the same glTF animation if they're pushed onto NLA tracks with the same name. When off, all the currently assigned actions become one glTF animation |
| `export_original_specular` | BOOLEAN | `False` | Export original glTF PBR Specular, instead of Blender Principled Shader Specular |
| `will_save_settings` | BOOLEAN | `False` | Store glTF export settings in the Blender project |
| `export_hierarchy_full_collections` | BOOLEAN | `False` | Export full hierarchy, including intermediate collections |
| `export_extra_animations` | BOOLEAN | `False` | Export additional animations. This feature is not standard and needs an external extension to be included in the glTF file |
| `export_loglevel` | INT [-2147483648..2147483647] | `-1` | Log Level |
| `filter_glob` | STRING | `*.glb` |  (hidden) |
Effective coupling rules applied in `execute()` **[V-src]**:
- `export_tangents` takes effect only if `export_normals` is on.
- `export_vertex_color='NONE'` forces `export_all_vertex_colors` and `export_active_vertex_color_when_no_material` to False. `export_vertex_color_name` is used only with `NAME`.
- `export_unused_images` and `export_unused_textures` take effect only with `export_materials='EXPORT'`.
- `use_active_collection_with_nested` takes effect only with `use_active_collection`.
- With `export_animations=False`, every animation option is ignored.
- `export_force_sampling=False` forces `export_def_bones` and `export_bake_animation` to False.
- `NLA_TRACKS` forces sampling on.
- `export_anim_scene_split_object` applies only in `SCENE` mode.
- `export_pointer_animation` and `export_convert_animation_pointer` apply only in `NLA_TRACKS` or `SCENE` mode.
- Any mode other than `ACTIONS` forces `export_merge_animation='NLA_TRACK'`.
- `export_anim_single_armature` applies only in `ACTIONS` mode.
- `export_skins=False` disables `export_all_influences` and `export_def_bones`.
- `export_morph=False` disables morph normals, morph tangents and morph animation.
- `export_try_sparse_sk=False` disables `export_try_omit_sparse_sk`.
- The `export_gltfpack_*` options are read only with `export_use_gltfpack`.
- `check_existing`, `filter_glob`, `ui_tab`, `will_save_settings` and `export_loglevel` are UI or bookkeeping properties. frap should accept them and ignore them.

### 9.2 `bpy.ops.import_scene.gltf` (20 properties)

| Property | Type | Default | Description |
|---|---|---|---|
| `filepath` | STRING | `""` | Filepath used for importing the file (hidden) |
| `export_import_convert_lighting_mode` | ENUM: `SPEC` / `COMPAT` / `RAW` | `'SPEC'` | Optional backwards compatibility for non-standard render engines. Applies to lights |
| `filter_glob` | STRING | `*.glb;*.gltf` |  (hidden) |
| `directory` | STRING | `""` |  (hidden) |
| `files` | COLLECTION | `None` |  |
| `loglevel` | INT [-2147483648..2147483647] | `0` | Log Level |
| `import_pack_images` | BOOLEAN | `True` | Pack all images into .blend file |
| `merge_vertices` | BOOLEAN | `False` | The glTF format requires discontinuous normals, UVs, and other vertex attributes to be stored as separate vertices, as required for rendering on typical graphics hardware. This option attempts to combine co-located vertices where possible. Currently cannot combine verts with different normals |
| `import_shading` | ENUM: `NORMALS` / `FLAT` / `SMOOTH` | `'NORMALS'` | How normals are computed during import |
| `bone_heuristic` | ENUM: `BLENDER` / `TEMPERANCE` / `FORTUNE` | `'BLENDER'` | Heuristic for placing bones. Tries to make bones pretty |
| `disable_bone_shape` | BOOLEAN | `False` | Do not create bone shapes |
| `bone_shape_scale_factor` | FLOAT [-3.4028234663852886e+38..3.4028234663852886e+38] | `1.0` | Scale factor for bone shapes |
| `guess_original_bind_pose` | BOOLEAN | `True` | Try to guess the original bind pose for skinned meshes from the inverse bind matrices. When off, use default/rest pose as bind pose |
| `import_webp_texture` | BOOLEAN | `False` | If a texture exists in WebP format, loads the WebP texture instead of the fallback PNG/JPEG one |
| `import_unused_materials` | BOOLEAN | `False` | Import materials & Images not assigned to any mesh |
| `import_select_created_objects` | BOOLEAN | `True` | Select created objects at the end of the import |
| `import_scene_extras` | BOOLEAN | `True` | Import scene extras as custom properties. Existing custom properties will be overwritten |
| `import_scene_as_collection` | BOOLEAN | `True` | Import the scene as a collection |
| `import_merge_material_slots` | BOOLEAN | `True` | Merge material slots when possible |
| `import_point_as_pointcloud` | BOOLEAN | `False` | Import mesh with only POINTS primitives as Point Cloud objects |
---

## Appendix A: Blender 5.2 RNA facts used by the mapping [V-rna]
- **Principled BSDF inputs (5.2), with defaults:**
  - Base Color (0.8, 0.8, 0.8, 1); Metallic 0; Roughness 0.5; IOR 1.5; Alpha 1; Thin Wall False; Normal; Weight 0; Diffuse Roughness 0.
  - Subsurface Weight 0, Subsurface Radius (1, 0.2, 0.1), Subsurface Scale 0.005, Subsurface IOR 1.4, Subsurface Anisotropy 0.
  - Specular IOR Level 0.5; Specular Tint (1,1,1,1); Anisotropic 0; Anisotropic Rotation 0; Tangent; Transmission Weight 0.
  - Coat Weight 0, Coat Roughness 0.03, Coat IOR 1.5, Coat Tint white, Coat Normal.
  - Sheen Weight 0, Sheen Roughness 0.5, Sheen Tint white.
  - Emission Color (1,1,1,1), Emission Strength 0.
  - Thin Film Thickness 0, Thin Film IOR 1.33.
- `Action` has `slots`, `layers`, `frame_range` and `use_frame_range`. It has **no** `fcurves`.
- `Material` has `surface_render_method ∈ {DITHERED, BLENDED}`, a legacy `blend_method`, `use_backface_culling`, `alpha_threshold` and `use_transparency_overlap`. `use_nodes` is deprecated and goes away in 6.0.
- `Light` has `exposure`, `normalize`, `use_temperature`, `temperature`, `use_custom_distance` and `cutoff_distance`. `energy`, `shadow_soft_size` and the spot fields live on the subtypes.
- `Camera` has `angle`, `angle_x`, `angle_y`, `sensor_fit`, `lens_unit`, `ortho_scale`, `clip_start` and `clip_end`.
- `Mesh` has `color_attributes` (current) and `vertex_colors` (legacy, still present).

## Appendix B: Verification runs (reproducible)
The scripts are in `docs/research/blender-probes/`. `exp2.py` to `exp4.py` expect `dumphelper.py` in the parent directory of `<outdir>`, so put the output directory next to it. `dumpprops.py` dumps the operator RNA to JSON, and `rna.py` checks RNA names.

Each run used `~/Applications/Blender-5.2.app/Contents/MacOS/Blender -b --factory-startup --python <script> -- <outdir>`. The scripts exported GLBs, then parsed the header, chunks and JSON.
1. **Default startup scene:**
   - Exported with defaults; then with cameras, lights and extras.
   - Checked the GLB header, the JSON padding (spaces), BIN = 840 B, the 24-vertex cube, the default material factors, and the camera and light conversions.
2. **Two-material cube:**
   - Material 1: an image texture plus an alpha link; an ORM image through Separate Color G and B; a Normal Map at 0.7; a UV Map node and a Mapping node; emission strength 5.
   - Material 2: transmission, coat, sheen, IOR 1.33 and Specular IOR Level 0.25, with backface culling on.
   - The cube also had a second UV layer and a BYTE_COLOR corner attribute.
   - Exported with defaults, with `export_vertex_color='ACTIVE'`, and with `export_tangents=True` plus JPEG.
3. **Animation scene:**
   - An object with location and rotation keys at frames 1 and 11; a shape key with value keys; a two-bone armature with a skinned cylinder and a quaternion pose key.
   - Exported with defaults, `export_force_sampling=False`, `NLA_TRACKS` and `SCENE`.
4. **Instancing and edge cases:**
   - Linked duplicates (`Cube`, `Cube.001`), natural name sort (`Object2` vs `Object10`), one-vertex shape key (sparse), Subdivision Surface with and without `export_apply`.

Keep these scripts as the seed of frap's golden-test corpus: same scene script, run in Blender and in frap, then structural comparison as described in §7.2.
