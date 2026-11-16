# View-transform LUTs (generated, do not edit)

Written by `tools/blender/bake_view_luts.py` from Blender 5.2.4 LTS (ARCHITECTURE §19.5):

```
$BLENDER_BIN -b --factory-startup --python tools/blender/bake_view_luts.py -- --out assets/luts
```

| File | Content |
|---|---|
| `agx.cube` | AgX, look None, sRGB display. 57³ lattice over Blender's own AgX shaper (Rec.709 → E-Gamut matrix, then OCIO `lg2` allocation `[-12.47393, 12.5260688117]`), so its nodes coincide with the nodes of Blender's `AgX_Base_sRGB.cube`. Values are Rec.1886-encoded; `# frap-post: rec1886_to_srgb` converts them to the sRGB display after tetrahedral interpolation, as Blender does. Every node was measured through `Image.save_render` (16-bit PNG, dither 0); no Blender LUT file is copied. |
| `heldout.json` | 400 random HDR colours (seed 20261116, never on a lattice) with Blender's 16-bit display outputs for Standard, Raw and AgX at several exposure/gamma settings, plus a dithered flat render that pins `render.dither_intensity`. `crates/frap-color/tests/heldout.rs` gates on CIEDE2000 < 1 against these colours only. |

The `# frap-*:` header comments of a `.cube` carry what frap needs to apply it
(`frap-view-transform`, `frap-look`, `frap-display`, `frap-matrix`, `frap-shaper`,
`frap-interpolation`, `frap-post`). Re-running the script reproduces both files byte for byte.
Standard and Raw are analytic in `frap-color` and need no LUT. More view transforms and looks
(Filmic, Khronos PBR Neutral, ACES, False Color, AgX looks) are added to `LUT_SPECS` by WP 5.3.
