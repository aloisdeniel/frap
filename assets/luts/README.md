# Colour-management data (generated, do not edit)

Written by `tools/blender/bake_view_luts.py` from Blender 5.2.4 LTS (ARCHITECTURE §19.5):

```
$BLENDER_BIN -b --factory-startup --python tools/blender/bake_view_luts.py -- --out assets/luts
```

| File | Content |
|---|---|
| `<family>.ops` (`standard`, `aces`, `pbr_neutral`, `agx`, `filmic`, `false_color`) | One `[display\|view\|look]` section per combination Blender renders (displays sRGB, Display P3, Rec.1886, Rec.2020; every view of frap's `ViewTransform` the display offers; every look Blender applies to the view), holding the op list of the OCIO processor Blender builds for it (`create_ocio_display_transform`: `LookTransform` into the look's process space, then `DisplayViewTransform`), read from Blender's bundled OpenColorIO 2.5 with `OPTIMIZATION_LOSSLESS`. `crates/frap-color/src/display/ocio.rs` interprets them with ports of OCIO's CPU renderers, including the `OPTIMIZATION_FAST_LOG_EXP_POW` approximations Blender's default processors use. |
| `*.cube`, `*.lut1d` | The LUTs those ops reference, read back from Blender's OCIO processors and named after their `datafiles/colormanagement` file: AgX base LUTs (`agx` = `AgX_Base_sRGB`, `agx_base_p3`, `agx_base_rec2020`), `luminance_compensation_bt2020` ("AgX Log"), `filmic_desat_33` and the `filmic_to_*` curves (Filmic and the generic contrast looks), `pbr_neutral`, `agx_false_color`. 3D LUTs list red fastest. `agx.cube` also carries `# frap-*:` headers so `frap_color::lut::baked("AgX", "None", "sRGB")` reads it as a single-LUT view. |
| `heldout.json` | 400 random HDR colours (seed 20261116) with Blender's 16-bit display outputs through `Image.save_render` (dither 0) for every baked combination plus exposure/gamma variants, and a dithered flat render that pins `render.dither_intensity`. `crates/frap-color/tests/heldout.rs` gates on CIEDE2000 < 1 against these renders. |

Re-running the script reproduces every file byte for byte. The HDR displays (Rec.2100-PQ/HLG)
are not baked (Blender re-encodes HDR image files to the 203-nit convention, which frap does not
implement yet).
