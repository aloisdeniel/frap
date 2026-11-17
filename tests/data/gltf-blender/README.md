# Blender glTF import fixtures (WP 1.17)

Written by `tools/blender/make_gltf_fixtures.py` with Blender 5.2.4 LTS:

```
$BLENDER_BIN -b --factory-startup -t 8 --python tools/blender/make_gltf_fixtures.py -- \
    --out tests/data/gltf-blender            # add --no-export to keep the committed GLBs
```

- `fixtures/<name>.glb`: small scenes built with bpy and exported by `export_scene.gltf`
  (cube, smooth sphere, hierarchy with an empty and a quaternion-rotated cone, multi-material,
  smooth sides with flat caps, image texture, two objects sharing a mesh, faces + loose edges +
  loose points). They are frap's own data (GPL-3.0-or-later).
- `fixtures/<name>.gltf`: hand-written glTF documents built by the same script
  (`GLTF_FIXTURES`). `empty_names.gltf` gives every node, mesh, camera, light, material, image
  and scene `"name": ""` (valid glTF that some third-party exporters write) plus extras on
  each: the add-on's falsy fallbacks (`Node_%d`, `Mesh_%d`, `Camera`, `Scene %d`, `Image_%d`)
  apply to "", while lights and materials test key presence.
- `dumps/<case>.json`: Blender's state after `read_factory_settings(use_empty=True)` and
  `import_scene.gltf(filepath=<source>, **settings)` on a fixture or a vendored Khronos sample
  (`tests/data/gltf-samples`, licences there). One top-level key per line: `source`,
  `settings`, `scenes`, `collections`, `objects`, `meshes`, `materials`, `images`, `cameras`,
  `lights`.

Dumped per ID: custom properties (glTF extras); per object also `rotation_euler`,
`matrix_parent_inverse` and material slot links; per image `filepath` relative to the source
file; per camera sensor size and shift; per light `use_shadow`/`shadow_soft_size`. Shape keys and
actions are not dumped. Material node graphs are dumped but compared only by
`material_graphs_match_blender`, which stays ignored until WP 1.11's `raise` lands.

## Khronos samples

Dumped (static content): Box, BoxInterleaved, BoxVertexColors, MeshPrimitiveModes,
MultipleScenes, SimpleMaterial, SimpleMeshes, SimpleSparseAccessor, Triangle,
TriangleWithoutIndices, TextureTransformTest, UnlitTest, EmissiveStrengthTest, Cameras,
PointLightIntensityTest, InterpolationTest (rest pose), SimpleMorph and AnimatedMorphCube
(glTF-Binary and glTF-Quantized / `KHR_mesh_quantization`; base mesh only).

Deferred (vendored but not dumped):

| Model | Why | Milestone |
| --- | --- | --- |
| SimpleSkin, RiggedSimple | skins (armatures, bones, vertex groups) | M6 |
| SimpleInstancing | `EXT_mesh_gpu_instancing` | M7 |
| AnimatedTriangle | Blender's object rotation is the evaluated animation, not the node's rest TRS | M6 |

`crates/frap-io-gltf/tests/import_vs_blender.rs` imports every case in-process and compares
it with its dump; `dumps_match_live_blender` re-dumps them when Blender is available and fails
on a stale file.
