# Khronos glTF sample subset

Small models from the Khronos [glTF-Sample-Assets](https://github.com/KhronosGroup/glTF-Sample-Assets)
repository (`main`, `Models/<name>/<variant>/`), vendored unmodified for the `frap-gltf` tests
(ARCHITECTURE §22.3, WP 1.5) and the importer tests that build on them (WP 1.17).

Each model directory keeps the upstream `LICENSE.md` and `metadata.json`, which are the
authoritative licence records. These files are **not** covered by frap's GPL-3.0-or-later licence;
they keep their own licences, listed below. CC-BY-4.0 models are credited to the owners named
here and in their `LICENSE.md`.

| Model | Variants | Licence | Owner |
|---|---|---|---|
| AnimatedMorphCube | glTF-Binary, glTF-Quantized | CC0-1.0 | Public |
| AnimatedTriangle | glTF-Embedded | CC0-1.0 | Public |
| Box | glTF-Binary | CC-BY-4.0 | Cesium |
| BoxInterleaved | glTF-Binary | CC-BY-4.0 | Cesium |
| BoxVertexColors | glTF-Binary | CC0-1.0 | Public |
| Cameras | glTF-Embedded | CC0-1.0 | Public |
| EmissiveStrengthTest | glTF-Binary | CC-BY-4.0 | AGI |
| InterpolationTest | glTF-Binary | CC0-1.0 | Public |
| LightVisibility | glTF-Binary | CC0-1.0 | Khronos Group |
| MeshPrimitiveModes | glTF-Embedded | CC0-1.0 | Public |
| MultipleScenes | glTF-Embedded | CC0-1.0 | Public |
| PointLightIntensityTest | glTF-Binary | CC0-1.0 | Public |
| RiggedSimple | glTF-Binary | CC-BY-4.0 | Cesium |
| SimpleInstancing | glTF-Binary | CC0-1.0 | Public |
| SimpleMaterial | glTF-Embedded | CC0-1.0 | Public |
| SimpleMeshes | glTF-Embedded | CC0-1.0 | Public |
| SimpleMorph | glTF-Embedded | CC0-1.0 | Public |
| SimpleSkin | glTF-Embedded | CC0-1.0 | Public |
| SimpleSparseAccessor | glTF-Embedded | CC-BY-4.0 | Public |
| TextureTransformTest | glTF | CC0-1.0 | Microsoft |
| Triangle | glTF | CC0-1.0 | Public |
| TriangleWithoutIndices | glTF-Embedded | CC0-1.0 | Public |
| UnlitTest | glTF-Binary | CC-BY-4.0 | Analytical Graphics, Inc. |

## What the subset covers

- Containers: `.gltf` with external `.bin`/`.png`, `.gltf` with data URIs, `.glb`.
- Accessors: interleaved (`byteStride`), sparse (with and without a base buffer view),
  KHR_mesh_quantization (`AnimatedMorphCube/glTF-Quantized`), normalized integers.
- Every primitive mode, POINTS to TRIANGLE_FAN (`MeshPrimitiveModes`).
- Morph targets, skins, animations (LINEAR, STEP, CUBICSPLINE: `InterpolationTest`), cameras,
  multiple scenes, vertex colours.
- Typed extensions: KHR_lights_punctual, KHR_texture_transform, KHR_materials_emissive_strength.
- Extensions frap-gltf does not type, which must round-trip untouched: KHR_materials_unlit,
  EXT_mesh_gpu_instancing, KHR_node_visibility, KHR_animation_pointer.

BoxTextured was left out on purpose: its texture carries a trademarked logo
(`LicenseRef-LegalMark-Cesium`).

To refresh, download the same `Models/<name>/<variant>/` files plus `LICENSE.md` and
`metadata.json` from the repository above and keep this table in sync.
