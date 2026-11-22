# Spec refresh: re-dumping Blender 5.2.x

frap's generated tables (`crates/*/src/generated/**`) come from the Blender dumps in
`spec/blender-5.2/` (ARCHITECTURE §4). `cargo xtask spec-refresh` re-runs the dump scripts
against a Blender 5.2.x, compares the result with the committed dumps, and reports every
difference together with what has to follow: regenerated code, `docs/OPERATORS.md` rows and
`.frap` migrations. Run it on every Blender 5.2.x patch release, before the golden suite moves
to that release, and after any change to a dump script (ARCHITECTURE §22.10 WP 8.9, §23
"Blender 5.3 API churn").

## Running it

```sh
cargo xtask spec-refresh --check [--blender <path>]   # re-dump, report, exit 1 on drift
cargo xtask spec-refresh [--blender <path>]           # re-dump, report, write spec/blender-5.2/
```

| Option | Meaning |
|---|---|
| `--check` | Never writes the spec. Fails when the API drifted. |
| `--blender <path>` | Any Blender 5.2.x. Without it: `$BLENDER_BIN`, then `blender` on `PATH`, then `~/Applications/Blender-5.2.app` (the golden harness's lookup). Other versions are refused: 5.3 gets its own `spec/blender-5.3/`. |
| `--out <dir>` | Scratch directory. Defaults to `<target>/spec-refresh/`. |
| `--seed <script.py>` | A Python script that runs before every dump script. Used for the seeded drift fixture. It needs `--check` (or another `--spec`). |
| `--spec <dir>` | The spec directory to compare and refresh. Defaults to `spec/blender-5.2/`. Tests use it. |

The scratch directory holds `dumps/` (the five families, laid out like `spec/blender-5.2/`),
`logs/<family>.log` (Blender's output), `report.md` (printed too) and `report.json`.

What runs: `<blender> -b --factory-startup --python-exit-code 1 [--python <seed>] --python
tools/blender/dump_<family>.py -- --out <scratch>/dumps` for `rna`, `ops`, `nodes`, `modifiers`
and `startup`. A Python exception fails the run. `spec/blender-5.2/primitives/`
(`dump_primitives.py`, WP 1.4) is a set of test fixtures, so it is not re-dumped here.

## How dumps are compared

1. **Bytes first.** On the same Blender build, the dumps are deterministic: a re-dump with
   5.2.4 is byte-identical to the committed spec (`live_check_reports_no_drift`).
2. **Version fields are not drift.** `blender_version`/`build_hash` of each `_meta.json` and
   `startup.json`'s `blender` section are compared on their own. When only these differ, the
   API did not change: `--check` passes with a warning. A refresh records the new version and
   hash, which also appear at the top of the report (both the dumping Blender's and the spec's).
3. **Structure.** Arrays of objects are matched by `idname`/`identifier`/`bl_idname`. A removed
   element and an added one whose contents are equal, apart from the key (or the key, `name`
   and `description`), count as a **rename**. String lists (`options`, `flags`, the `_meta.json`
   lists) are compared as ordered sets. Any other array is compared as one value.
4. **Classification.** Each difference becomes one line of the report:

| Category | Example |
|---|---|
| `operator added` / `operator removed` | `uv.lightmap_pack` unregistered |
| `struct` / `node type` / `modifier type` / `namespace` `added` / `removed` | a file of `rna/`, `nodes/`, `modifiers/`, `ops/` appeared or disappeared |
| `property added` / `removed` / `renamed`, `function …`, `parameter …`, `socket …` | `object.select_pattern` `case_sensitive` → `match_case` |
| `enum item added` / `removed` / `renamed` / `changed`, `dynamic enum changed` | `uv.follow_active_quads` `mode` + `UNIFORM` |
| `default changed` | an operator or RNA property default, a socket `default_value`, a modifier value right after `modifiers.new` |
| `fresh value changed` | `rna/*.json` `fresh.differs_from_default` (the values of a newly created ID) |
| `poll cell changed` / `added` / `removed` | the A–E × VIEW_3D poll matrix of `dump_ops.py` |
| `flag added` / `removed`, `bl_options changed`, `range changed`, `type changed`, `socket availability changed`, `context member …` | |
| `meta list added` / `removed`, `meta changed` | `_meta.json` struct, namespace, node and modifier type lists, the RNA enums |
| `factory state changed` | `startup.json` |
| `order changed` | the same elements in a different order (enum values and generated code follow dump order) |
| `formatting changed` | same JSON, different bytes (number format, key order) |

## The routine

1. **Check.** `cargo xtask spec-refresh --check --blender <new 5.2.x>`. A pass with only a version
   warning still needs a refresh, to record the new version.
2. **Review** `report.md`: each drift item, and the three follow-up sections described below.
   Drift on a 5.2.x patch release is unusual (an LTS keeps its API), so every item needs an
   explanation before it is accepted.
3. **Refresh.** `cargo xtask spec-refresh --blender <new 5.2.x>` writes the changed and new
   files into `spec/blender-5.2/` and deletes the ones that are no longer dumped. Review them
   with `git diff spec/blender-5.2/`.
4. **Regenerate.** Run `cargo xtask codegen`, then `cargo xtask codegen --check`. The report's
   *codegen* section has already listed the generated files that change. That list comes from a
   dry run on a shadow workspace: the committed overlays, `spec/frap/` and OPERATORS.md, with the
   re-dump as `spec/blender-5.2/`. The dry run first deletes the OPERATORS.md rows of removed
   operators, otherwise codegen stops at the first one. If codegen fails, the error tells what
   to fix first: a new dynamic enum without a `spec/overlay/dynamic_enums.toml` entry, an overlay
   entry that matches nothing, or a `compat_4x.toml` member that now exists.
5. **Test.** `cargo test -p xtask`, the `frap-spec-*` drift tests (`cargo test -p frap-spec-rna
   --test drift`, the `frap-spec-ops-*` ones) and the `--describe` snapshots. After a
   `startup.json` change, also run `cargo test -p frap-scene --test startup`. Then the full gate,
   and `cargo xtask golden live` against the new Blender.
6. **Update `docs/OPERATORS.md`.** The report gives one line per drifted operator, struct,
   modifier type or shader node, with its row (`OPERATORS.md:<line> §<section>`) and status:
   - *removed*: delete the row. If frap implements it, unregister frap's operator, remove its
     snapshot and goldens, and add a `docs/deviations.md` note if agents may still call it.
   - *implemented* (`[x]`) and changed: update the implementation, its `--describe` snapshot and
     golden cases. Untick the row until they pass again (rule 2).
   - *planned* (`[ ]`): nothing to do now. The WP that implements it works from the refreshed
     spec.
   - *not listed*: OUT by rule (3). Add a row if a new operator is in scope.
7. **Write the `.frap` migrations.** `scene.json` keys and enum values are RNA identifiers (§15).
   The report lists every RNA property, enum item or struct that was renamed or removed. For each
   one that frap persists, add `crates/frap-project/src/migrate/steps/v{N}_to_v{N+1}.rs`
   (`pub const MIGRATION`, appended to `MIGRATIONS`, using `rename_key`/`for_each_id`) and bump
   `SCHEMA_VERSION`. These need no migration:
   - operator properties (they are not persisted);
   - function parameters;
   - changed defaults (`.frap` writes every value of an implemented property).

   `blender_api` stays `"5.2"` for every 5.2.x. Moving to Blender 5.3 means a new
   `spec/blender-5.3/`, a `blender_api` bump, and migrations keyed on it.
8. Commit the dumps, the generated code and the follow-ups together.

## Rules for the dumps

- **No hand edits.** A hand edit makes the next refresh report drift. A fact that bpy cannot
  show goes into the dump script, with its Blender source reference, or into `spec/overlay/`.
  Example: `dump_rna.py` `DYNAMIC_PARAMS` lists the `PROP_DYNAMIC` function parameters that bpy
  reports as fixed one-element arrays (`VertexGroup.add/remove` `index`). WP 6.3 had recorded
  them by hand.
- Changing a dump script is a spec refresh: run the routine and commit its output.

## Seeded drift fixture

`tools/blender/spec_refresh_seed.py` re-registers Python-defined operators before each dump,
so that the real Blender produces one drift of each kind:

| Kind | Operator | Change |
|---|---|---|
| renamed property | `object.select_pattern` | `case_sensitive` becomes `match_case` |
| new poll cell | `object.select_pattern` | poll also False in Pose Mode (D, DV) |
| changed default | `object.subdivision_set` | `level` default 1 becomes 2 |
| new enum item | `uv.follow_active_quads` | `mode` gains `UNIFORM` |
| removed operator | `uv.lightmap_pack` | unregistered |

The expected report, with its codegen, OPERATORS.md and migration follow-ups, is
`xtask/src/spec_refresh/fixtures/seeded.expected.md`. Two tests check it:

- `seeded_drift_gives_the_expected_report` is hermetic and runs on every `cargo test`. It applies
  the same five changes to the committed dumps.
- `live_seeded_drift_gives_the_expected_report` runs the seed script through Blender.

The comparison ignores OPERATORS.md line numbers and the file count, which other work changes in
passing. `FRAP_BLESS=1` rewrites the fixture. The Blender tests are `#[ignore]`d:
`cargo test -p xtask spec_refresh -- --include-ignored`.

## CI

`.github/workflows/spec-refresh.yml` is a manual (`workflow_dispatch`) job on ubuntu-24.04. Its
inputs are the Blender release to download (`5.2.x`) and the mode:

- `check`: fail on drift;
- `refresh`: upload the refreshed `spec/blender-5.2/` as an artifact, to commit by hand.

For 5.2.4 it also runs the Blender tests above. The report goes to the job summary, and the
report and logs are uploaded as an artifact.
