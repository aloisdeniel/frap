# spec/: Blender facts frap is generated from

Source of truth for every generated table (ARCHITECTURE §4). Nothing here is edited by hand
except `overlay/`, `frap/` and `current_milestone`.

| Path | Content | Written by |
|---|---|---|
| `blender-5.2/rna/<Struct>.json` | `bl_rna` of every struct frap may implement (515 structs: the OPERATORS.md §D roots, every `Modifier`, `ShaderNode*`, `NodeSocket*` and `Light` type, the glTF add-on preferences, closed over bases and pointer/collection targets; UI, editor, physics and tool structs pruned), with `fresh.differs_from_default` per creatable struct and `context_members` on `Context` | `tools/blender/dump_rna.py` |
| `blender-5.2/ops/<ns>.json` | every operator of the 20 curated namespaces (1,073): props, options, macro steps, poll matrix A–E with and without a VIEW_3D override | `tools/blender/dump_ops.py` |
| `blender-5.2/nodes/<bl_idname>.json` | every shader node: default name/width, sockets with defaults, fresh property values, socket availability per enum value | `tools/blender/dump_nodes.py` |
| `blender-5.2/modifiers/<TYPE>.json` | every mesh modifier type: `modifiers.new` values, `modifier_add` default name | `tools/blender/dump_modifiers.py` |
| `blender-5.2/startup.json` | the complete factory state: every ID (generic RNA serialisation), exact mesh arrays of the default cube, the Layout viewport's `RegionView3D`/`SpaceView3D` | `tools/blender/dump_startup.py` |
| `blender-5.2/primitives/` | primitive reference meshes (test fixtures) | `tools/blender/dump_primitives.py` (WP 1.4) |
| `frap/ops/<ns>.json` | hand-written specs of frap's own operators (`frap.*`), in the dump schema, `poll: null` | the WP implementing the operator |
| `overlay/*.toml` | facts bpy cannot dump: dynamic enum supersets, Python-only members, removed 4.x members, frap-only options and their audit table, context member types, float storage width | hand-written, each entry with its source |
| `current_milestone` | operators with OPERATORS.md `M` ≤ this get typed props structs; all others go to the catalog | the milestone's W0 integrator |
| `m0/` | the M0 walking-skeleton subset, checked against a live Blender by `frap-cli`'s `blender_live` test | WP 0.1 |

## Regenerating

```sh
B=$BLENDER_BIN   # Blender 5.2.4 LTS
for s in rna ops nodes modifiers startup; do
  $B -b --factory-startup --python tools/blender/dump_$s.py -- --out spec/blender-5.2/
done
cargo xtask codegen          # dumps + overlays -> crates/*/src/generated/**
cargo xtask codegen --check  # CI: fails on any difference or stale generated file
```

The dumps are deterministic (two runs are byte-identical): per-session values are excluded
(`session_uid`, ID names, temp paths are written `$TEMPDIR/…`, the machine's thread count is not
recorded). `dump_rna.py`/`dump_ops.py` switch the glTF add-on preference
`KHR_materials_variants_ui` on first (§4.1); `_meta.json` records it under `addon_prefs`.

Codegen fails on a dynamic enum without items that has no `overlay/dynamic_enums.toml` entry, on
an overlay entry that matches nothing, on a `frap_*` option that duplicates a Blender property
(§4.2 audit), on a `compat_4x.toml` member that exists in 5.2, and on any generated name collision.
