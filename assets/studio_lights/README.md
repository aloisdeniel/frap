# Studio lights (Workbench `light='STUDIO'`)

Owner: WP 1.7. Used by `crates/frap-render/src/workbench/studio_light.rs` (embedded with
`include_str!`).

| File | Source | Licence |
|---|---|---|
| `Default.sl` | Blender 5.2.4 LTS built-in rig "Default" (hard-coded in `source/blender/blenkernel/intern/studiolight.cc`), dumped through `bpy.context.preferences.studio_lights["Default"]` | GPL-2.0-or-later (Blender) |
| `basic.sl`, `outdoor.sl`, `paint.sl`, `rim.sl`, `studio.sl` | Blender 5.2.4 LTS `release/datafiles/studiolights/studio/*.sl`, re-written by the bake script; byte-identical to the shipped files (checked with `cmp`) | GPL-2.0-or-later (Blender) |

Regenerate:

```
Blender -b --factory-startup --python assets/studio_lights/bake_studio_lights.py -- assets/studio_lights
```

Format (Blender's own `.sl` writer): `version 1`, `light_ambient.{x,y,z}`, then for each of the
four solid lights `light[i].flag` (`use`), `.smooth` (wrap), `.col.*` (diffuse), `.spec.*`
(specular), `.vec.*` (direction, view space when `use_world_space_lighting` is off). All five
datafile rigs and "Default" report `has_specular_highlight_pass = True`.
