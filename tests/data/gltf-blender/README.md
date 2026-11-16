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
- `dumps/<case>.json`: Blender's state after `read_factory_settings(use_empty=True)` and
  `import_scene.gltf(filepath=<source>, **settings)` on a fixture or a vendored Khronos sample
  (`tests/data/gltf-samples`, licences there). One top-level key per line: `source`,
  `settings`, `scenes`, `collections`, `objects`, `meshes`, `materials`, `images`, `cameras`,
  `lights`.

`crates/frap-io-gltf/tests/import_vs_blender.rs` imports every case in-process and compares
it with its dump; `dumps_match_live_blender` re-dumps them when Blender is available and fails
on a stale file.
