# Golden tests against Blender 5.2

Owned by WP 1.8 (harness). Each WP owns the cases and fixtures of its own area directories
(ARCHITECTURE §22.1). Normative description: ARCHITECTURE §20.2–§20.5.

```
tests/golden/
  props.toml                    property list read by BOTH dumpers
  cases/<area>/<case>.py        golden cases (bpy-line subset, run by frap and by Blender)
  fixtures/<area>/<case>.blender.json   Blender's dump, committed (+ copied .glb/.png)
tools/blender/
  golden_runner.py              Blender side: runs a case, NORMALISE list (mirrors docs/deviations.md)
  dump_state.py                 Blender side: writes GoldenState JSON from props.toml
  emulations.py                 Blender implementations of frap-only operators (bpy.ops.frap.*)
crates/frap-golden/             GoldenState, case parser, frap runner, comparators, bless, CLI
```

## Writing a case

```python
# golden: startup=factory compare=objects,mesh:Cube.001,context tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))
#@frap    bpy.ops.frap.select_elements(type='FACE', indices=[5])
#@blender import bmesh; bm = bmesh.from_edit_mesh(bpy.context.object.data)
bpy.ops.mesh.primitive_cube_add(sizee=2)        # TypeError on both sides (T0)
```

Header (`# golden:` comment lines before the first statement; several are merged):

| Key | Values | Default |
|---|---|---|
| `startup` | `factory`, `empty` | `factory` |
| `compare` | comma list: `objects`, `mesh:<mesh name>`, `evaluated:<object name>`, `materials`, `anim:<action>`, `glb:<path>`, `image:<path>`, `context` | line outcomes only |
| `tier` | `T0` (outcomes, RNA values) … `T4` (images) | `T0` |
| `tol` | float tolerance (absolute below 1, relative above) | `1e-5` |
| `order` | `exact` / `canonical`: mesh comparator override | exact up to T1, canonical from T2 |
| `ssim` | minimum SSIM for `image:` | `0.995` (frap-blessed self-goldens) |
| `check` | T3 invariants: `manifold`, `euler:<n>`, `verts:<a>-<b>`, `edges:…`, `faces:…`, `uv01`, `uv_no_overlap`, `uv_area:<a>-<b>` | none |
| `skip_blender` | frap-only case: never blessed, only has to run | off |

Rules:

- Every line is a statement both tools accept (the bpy-line subset of §11.3). Each source line
  is one outcome: `ok`, or exception class + message. An operator returning `{'CANCELLED'}` is
  not ok.
- `#@frap` lines run only in frap, `#@blender` lines only in Blender (headless-impossible ops,
  frap selection helpers, the loopcut recipe). They keep their line numbers and must succeed on
  their side; they are not compared.
- A frap-only operator on a shared line works when `tools/blender/emulations.py::EMULATED`
  emulates it (`bpy.ops.frap.select_elements`, `bpy.ops.frap.render_contact_sheet`).
- Output paths are relative (`out/part.glb`): each run has its own work directory. Never `//`
  (D-08), never `CYCLES` (D-12), no positional operator arguments (D-01).
- Edit-mode state (`selection`, `active`, `select_history`) is dumped only for meshes of
  objects in Edit Mode.

## Running

```sh
cargo test -p frap-golden                    # frap vs committed fixtures (no Blender needed)
FRAP_GOLDEN_FILTER='primitives/**' cargo test -p frap-golden --test golden
cargo xtask golden bless --filter 'area/*'   # run Blender, (re)write fixtures; commit them
cargo xtask golden live                      # FRAP_GOLDEN_LIVE=1: Blender re-runs, fails on drift
cargo xtask golden check | report            # pass/fail per case | operator x tier matrix
cargo xtask golden diff tests/golden/cases/harness/cube_add.py   # both tools live, every mismatch
```

Blender is found at `$BLENDER_BIN`, then `blender` on `PATH`, then
`~/Applications/Blender-5.2.app/Contents/MacOS/Blender`. Without Blender, live tests print a
`NOTICE: SKIPPED …` line and pass; fixture comparisons still run.

A fixture is stamped with Blender's version and build hash; a live run with another build fails
(drift alarm). Bless only with Blender 5.2.4 LTS (`cd5dc428cce1`).

## How the two sides are dumped

Both dumpers read the properties listed in `props.toml`:

- Blender: `dump_state.py` with the Python API, after the runner's normalisations
  (`golden_runner.py::NORMALISE`, kept equal to `docs/deviations.md` §1 by a test): every
  statement runs in a VIEW_3D `temp_override` (D-10); `view_layer.update()` before derived reads
  and the dump unless time is dirty (D-04, no time normalisation); `update_from_editmode()`
  before mesh reads in Edit Mode (D-05); `Dots Stroke` skipped (D-11).
- frap: the same paths as statements executed by the session (`….matrix_world`,
  `….vertices.foreach_get('co')`), so goldens also test RNA and hooks. A datablock whose RNA
  read fails because its binding has not landed yet is dumped natively from `Main` and counted
  as an *RNA fallback* (printed by the test; `FRAP_GOLDEN_STRICT_RNA=1` makes fallbacks
  failures).

## Comparators (`crates/frap-golden/src/compare/`)

| File | Used for | Method |
|---|---|---|
| `lines.rs` | always | same ok / exception class; message equal after normalising quotes, whitespace, paths |
| `rna_values.rs` | `objects`, `context`, `materials`, `anim:` | every field, floats within `tol` |
| `mesh_exact.rs` | `mesh:`/`evaluated:` at T0–T1 | index by index |
| `mesh_canonical.rs` | `mesh:`/`evaluated:` from T2 | vertices matched on a 1e-4 grid, faces as rotation-normalised cycles, corner data per (face, corner) |
| `invariants.rs` | `check=` (T3) | on both meshes |
| `gltf_canonical.rs` | `glb:` | canonical glTF (decoded accessors, triangle multisets, names, defaults); `asset.generator` ignored |
| `gltf_roundtrip.rs` | WP 1.23 | the §17.2 round-trip law on `tests/data/gltf-roundtrip/allow.toml` (`[[file]] path = …`) |
| `image_ssim.rs` | `image:` (T4) | SSIM ≥ `ssim` and mean ΔE ≤ 2 in the centre crop |

The seed cases in `cases/harness/` exercise the harness itself.
