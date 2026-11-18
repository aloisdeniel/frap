# Matcaps (Workbench `light='MATCAP'`)

Owner: WP 2.3. Used by `crates/frap-render/src/workbench/matcap.rs` (embedded with
`include_bytes!`, decoded with `frap_imbuf::decode` on first use).

| Files | Source | Licence |
|---|---|---|
| `<name>.diffuse.exr`, `<name>.specular.exr` | Blender 5.2.4 LTS `release/datafiles/studiolights/matcap/<name>.exr` (the 27 matcaps of `bpy.context.preferences.studio_lights` with `type == 'MATCAP'`), passes split and reduced by `bake_matcaps.py` | CC0 / public domain (`release/datafiles/studiolights/matcap/license.txt`: "These matcap images are licensed as CC0 or public domain.") |
| `MANIFEST.txt` | The matcap names in Blender's `preferences.studio_lights` order, each with the passes it has | generated |

Regenerate:

```
Blender -b --factory-startup --python assets/matcaps/bake_matcaps.py -- assets/matcaps
```

What the bake does, and why:

- Blender's matcaps are multilayer, 512 x 512, half-float, DWAA-compressed EXRs with a `diffuse`
  and a `specular` pass (`check_normal+y.exr` is single-layer: diffuse only, no specular, so
  `use_specular` is off for it). Blender's Workbench samples only `.rgb` of each pass.
- DWAA is lossy and its decoded noise compresses badly losslessly (17 MB at full size), so each
  pass is reduced to 256 x 256 with a 2 x 2 box average in linear float and written as a
  single-layer RGB half-float ZIP EXR. Blender renders with the reduced matcaps (loaded as user
  matcaps) differ from renders with the shipped ones by at most 2/255 per channel at 640 x 360
  (basic_bright, check_rim_light, metal_carpaint). This is a deviation from Blender; WP 2.3 hands it
  to the integrator for `docs/deviations.md` (owned by the integrator, not this package).
- The factory `shading.studio_light` in MATCAP mode is `basic_bright.exr` (DNA default
  `basic_1.exr` no longer ships, so Blender falls back to the first matcap; verified on 5.2.4).
