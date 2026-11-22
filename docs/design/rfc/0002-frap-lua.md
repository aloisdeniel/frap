# RFC 0002: a Lua front-end over the Session API (WP 8.10 spike)

Status: **proposed** (for the M8 integrator). Author: WP 8.10. Date: 2026-11-22. Blender: 5.2.4 LTS
(`cd5dc428cce1`). mlua 0.12.2, Lua 5.4 (vendored `lua-src` 551.0.2); Luau 0.740 (`luau0-src` 0.22.0) for the
comparison only.

**Recommendation:** `frap-cli` should **not** ship a Lua front-end in M9. Keep `crates/frap-lua` as it is: a
spike behind the off-by-default `lua` feature, which nothing depends on. If a scripting front-end is wanted
later, use **Lua 5.4** (mlua `lua54` + `vendored`), not Luau, and add the four Session API changes listed in
§6 first. The reasons are in §7.

## 1. What was built

`crates/frap-lua` (layer L12, ARCHITECTURE §11.6). With `--features lua` it provides `LuaFrontend`, a Lua 5.4
state bound to one `Session`:

| File | Content |
|---|---|
| `src/front.rs` | `LuaFrontend::{new, with_config, exec, take_responses, take_output, with_session, into_session}`. Rust primitives `invoke`, `get`, `set`, `call`, `poll`, `fail`, `print` and `timed_out`. Conversions Lua → `Input` and response JSON → Lua. The deadline hook and the heap limit. |
| `src/prelude.lua` | The `bpy` proxies, the `frap` helpers, `None`, `Set`, and the sandbox wrappers. It runs once per state and receives the primitive table as an argument, so the primitives are never globals. |
| `src/errors.rs` | The codes the front-end raises itself (§4). |
| `examples/frap_lua.rs` | A sketch of `frap --lua`: one JSON line per session call, then the `print` lines, then a done record. |
| `tests/lua_frontend.rs` | The acceptance tests (§5), including the M1 exit transcript and the Blender reference. |
| `tests/default_build.rs` | Checks that the default build is unaffected (runs with and without the feature). |

Every scene access goes through the four §11.6 entry points. Nothing below `frap-session` changed:

| Lua | Session call |
|---|---|
| `bpy.ops.mesh.primitive_cube_add{size=2, location={0, 0, 1}}` | `invoke("mesh.primitive_cube_add", {location: Tuple, size: Int})` |
| `local ob = bpy.data.objects['Cube']`, `ob.location.x`, `bpy.context.object` | one `get` per hop: `bpy.data.objects`, `bpy.data.objects['Cube']`, … |
| `ob.location = {1, 2, 3}`, `ob.location.x = 5`, `bpy.context.scene.camera = cam` | `set(path, value)` |
| `bpy.data.materials.new("Steel")`, `ob:select_set(false)`, `coll:link(ob)` | `get` (gives `$func`), then `call(path, args, kwargs)` |
| `bpy.ops.object.delete.poll()` | `exec_source("bpy.ops.object.delete.poll()")`. The Session API has no `poll`; the idname is checked to be `ident.ident` first. |
| `frap.get/set/call/invoke(path…)` | the same calls, with a path string |

## 2. Language mapping

| Python (`bpy`) | Lua (`frap-lua`) | Why |
|---|---|---|
| `f(a, b=1)` | `f(a, {b=1})`, or `f{b=1}` when there are only keywords | Lua has no keyword arguments. A trailing plain table with at least one string key holds the keywords, and its sequence part continues the positional arguments. |
| `bpy.ops.x.y(k=v)` | `bpy.ops.x.y{k=v}` | Operators take keywords only. A positional argument raises `TypeError` `OP_ARG_POSITIONAL`. |
| keyword order | sorted by name | Lua tables have no order. Sorting makes the same table always produce the same call and the same first error. |
| `obj.method(x)` | `obj:method(x)` or `obj.method(x)` | A function proxy remembers its owner, so a colon call drops the owner from the arguments. `ob.select_set(ob)` (the owner passed as the first argument) cannot be told apart from a colon call; `frap.call` covers that case. |
| `None` | `None` (a sentinel) | Lua tables cannot hold `nil`, so `objects.new{name='E', object_data=None}` needs a value that is not `nil`. A read that returns `None` gives `nil` at the top level and the sentinel inside a sequence. |
| `{'VERT', 'FACE'}` (set) | `Set{'VERT', 'FACE'}` | Enum-flag reads come back as `Set` objects (`s.VERT` → `true`). A sequence table is a tuple. |
| `(1, 2, 3)`, `[1, 2, 3]` | `{1, 2, 3}` | A tuple. A table with string keys is a dict. A table that mixes both is refused (`LUA_VALUE`). |
| `int` / `float` | Lua 5.4 integer / float | The subtypes map onto `Input::Int`/`Input::Float` one to one, so Blender-exact coercion (`expected an int type, not float`) behaves the same in both front-ends (§7). |
| `objects[0]`, `materials[0]` | `objects[0]`, `materials[0]` | A proxy stands for a Blender path, so its indices are Blender's (0-based). Plain tables returned as values (`frap.value(ob.location)`, `context.selected_objects`) are Lua sequences (1-based). This is the most surprising rule of the mapping. |
| `ob.location.x = 1` writes through | the same | A whole array property is read as an *array proxy* (index, `.x/.y/.z/.w/.r/.g/.b/.a`, `#`, assignment). `frap.value` gives the plain sequence. |
| `for ob in bpy.data.objects` | `for i, ob in pairs(bpy.data.objects)` | `__pairs` and `frap.iter` yield the Blender index and the item. |
| `objects['new']` versus `objects.new` | `objects.new` is the method; an item named like a method needs `frap.get("bpy.data.objects['new']")` | Lua has a single `t[k]` operator. On a collection, a key that is an identifier is tried as an attribute first, and as an item if the attribute does not exist (`AttributeError`). |
| `print(a, b)` | `print(a, b)` | The front-end collects the lines (`take_output`); nothing is written to stdout. |

A struct proxy holds the *repr path* that `get` answered (`bpy.context.object` becomes
`bpy.data.objects['Cube']`), so `local ob = bpy.context.object` keeps pointing at the same object after the
active object changes, as a Python reference does. Collections, array proxies and functions also hold the
repr path; a function keeps the textual path it was reached through.

## 3. Sandbox and limits

- **Standard libraries.** `base`, `table`, `string`, `math`, `utf8` and `coroutine` (`StdLib` allow-list,
  `Lua::new_with`). There is no `os`, `io`, `package`/`require` (so no C module loading) and no `debug`. The
  prelude also removes `dofile`, `loadfile`, `collectgarbage`, `warn` and `string.dump`, and forces `load`
  to text mode, because binary chunks can break the VM's invariants. The test
  `the_sandbox_has_no_os_io_or_module_loading` checks each of these.
- **Proxies cannot be tampered with.** Their metatables carry `__metatable`, so `setmetatable` raises and
  `getmetatable` returns a string. Their data sits in a weak side table, so `rawget(bpy.data, 'objects')` is
  `nil`.
- **`--stmt-timeout`.** One `exec` (one chunk) is the unit, with deadline = start + `Limits::stmt_timeout`. A
  global hook (all coroutines) runs every 1000 instructions and raises once the deadline has passed. User
  code cannot get past it: `pcall`, `xpcall`, `coroutine.resume` and `coroutine.close` are wrapped to
  re-raise once the deadline has passed, and `exec` reports `RuntimeError` `TIMEOUT` with
  `detail.stmt_timeout_ms`, whatever error the chunk ended with. Each session call also has its own
  deadline, set by the session's budget scope. Six loop shapes are tested (plain, two `pcall`-swallowing
  ones, two coroutine ones, one with a session call in the loop); each stops within the test's bound after
  a 200 ms deadline.
- **Heap.** `LuaConfig::memory_limit` defaults to 256 MiB of Lua heap. Exceeding it raises `RuntimeError`
  `LIMIT_EXCEEDED`, and the state stays usable afterwards.
- **Re-entrancy.** A `__gc` metamethod that calls `bpy` while a primitive holds the session gets a Lua error,
  not a panic (`try_borrow_mut`). The deadline lives outside the `RefCell`, so the hook never borrows.

**Gaps (not closed by the spike):**

1. C functions are not interruptible. A pathological `string.find`/`gsub` pattern on a large subject runs
   to the end before the hook can fire. Luau has the same gap. Closing it would mean bounded replacements
   of the pattern functions.
2. **A chunk is not a transaction.** Each session call commits or rolls back on its own, so an error in
   the middle of a chunk leaves the calls before it committed (the hint says so). `Session::transact` is
   crate-private.
3. A timed-out chunk does not undo the session calls that finished before the deadline (same reason).

## 4. Errors

A failed session call becomes a Lua error whose value is a table `{exception, code, message, hint, detail}`
with `__tostring` = `"<exception>: <message>"`. `pcall` sees exactly that table, and `frap.is_error(e)` tells
it apart from other errors. `exec` returns the session's original `FrapError` (kept on the Rust side under
the table's index) with `detail.lua_line` and `detail.lua_where`. The innermost user frame is found without
the debug library by asking `error` for the position of successive levels. The front-end raises six codes of
its own:

| Code | Exception | When |
|---|---|---|
| `LUA_ERROR` | RuntimeError | a plain Lua error (`attempt to index a nil value`, `error('x')`), with its `lua:<line>:` position |
| `LUA_VALUE` | TypeError | a value with no Python counterpart (a function, a mixed table, a cycle or nesting deeper than 32, non-UTF-8) |
| `SYNTAX` | SyntaxError | a Lua syntax error (the code frap-lang uses for Python) |
| `OP_ARG_POSITIONAL` | TypeError | a positional operator argument |
| `TIMEOUT` | RuntimeError | `--stmt-timeout` (same message and detail as the session's) |
| `LIMIT_EXCEEDED` | RuntimeError | the Lua heap limit |

`errors_carry_the_sessions_exception_code_and_message` runs 19 failing statements, each in Python through
`Session::exec_source` and in Lua, and requires the same `(exception, code, message)` from both. They cover
unknown operators, poll failures, bad keywords, types, enums and lengths, the read-only context, missing
keys and indices, function argument errors and I/O errors. The Python answers are the ones the M1 goldens
pin to Blender 5.2.4, so Lua errors match Blender's wording by transitivity.

## 5. Acceptance evidence

`cargo test -p frap-lua --features lua` (macOS arm64; add `gpu` to render): 11 tests in
`lua_frontend.rs` (plus the ignored measurement of §6) and 4 in `default_build.rs`:

| Acceptance item (§22.10, 8.10) | Evidence |
|---|---|
| `bpy.ops.<ns>.<op>{…}`, `bpy.data` reads and writes, `bpy.context` map onto `Session::{invoke, get, set, call}` | `ops_data_and_context_map_onto_the_session` (operators, reads, write-through arrays, dot and colon calls, keyword calls, `None`, `Set`, `poll`/`idname`, iteration, the raw `frap.*` API, the response log). `globals_persist_between_chunks_and_responses_are_ordered`. |
| Errors as Lua errors carrying `exception`, `code`, `message` | `errors_carry_the_sessions_exception_code_and_message`, `a_failed_call_leaves_the_scene_unchanged_and_stops_the_chunk`, `lua_errors_and_bad_values_are_frap_errors` |
| Sandbox without `os`, `io` and C module loading | `the_sandbox_has_no_os_io_or_module_loading`, `the_lua_heap_is_limited` |
| `--stmt-timeout` honoured | `stmt_timeout_stops_a_chunk` |
| The M1 exit transcript in Lua reaches the same state | `the_m1_exit_transcript_in_lua_reaches_the_same_state`. `tests/data/m1_transcript.lua` is the 19-statement transcript, statement for statement (the render runs under `pcall`, so that a missing adapter answers like the Python run). It runs after the Python transcript in the same directory, and the two `Session::state_dump()`s are equal once process-wide ID counters (`IdKey`, `session_uid`, `generation`) are renumbered. Without a GPU adapter both front-ends answer `GPU_UNAVAILABLE` at the render and nothing else differs. |
| … and that state is Blender's | `the_m1_exit_transcript_in_lua_matches_blender`: `tests/data/dump_state.lua` prints 23 facts (objects, transforms, data names, mesh counts, material slots, colours, metallic before and after the GLB round trip, camera, engine, output path, active object). They match `tests/data/m1_transcript_blender.txt` (Blender 5.2.4 running `tests/blender/m1_transcript_state.py`) to 1e-6. `the_blender_reference_is_current` reruns Blender when it is installed and requires byte-identical facts (`FRAP_REQUIRE_BLENDER=1` turns a skip into a failure). |
| Default build and `cc`-only container unaffected | `default_build.rs`: the `lua` feature is off by default and owns every dependency except `frap-session`. No workspace manifest names `frap-lua`. `cargo tree -e normal,build` of `frap-cli` and of `frap-lua` without features contains no `mlua`, `mlua-sys`, `lua-src`, `luau0-src` or `frap-lua`. `cargo xtask check-deps` passes. |

## 6. Measurements, and what the Session API lacks

**Cost of a proxy walk** (`measure_lua_against_statements`, ignored test, debug profile, 2000 iterations).
These are per statement, and only the ratios mean anything:

| | Python statement | Lua |
|---|---|---|
| read `bpy.data.objects['Cube'].location.x` | 27.6 µs (1 statement) | 255 µs (4 `get`s); 134 µs from a cached `ob` (2 `get`s) |
| `….location = (1, 2, 3)` | 177 µs | 140 µs (1 `set`) |

Writes and calls cost the same as a statement. A read costs one full `get` per hop, and every `get` builds a
whole response envelope (context, edit-mesh block, undo bookkeeping) only for the front-end to discard it.
Four Session API changes would make a shipped Lua front-end cheap and exact:

1. **`Session::resolve(path) -> kind + repr`**, without a response envelope or undo hooks, for the
   intermediate hops. The walk would then cost one session call per value, as Python does.
2. **`Session::poll(idname)`**, instead of formatting a statement.
3. **A source line on `invoke`/`get`/`set`/`call`**. Today every response says `line: 1`, and navigation
   reads consume `seq` numbers, so the example's responses go `seq` 1…5, then 24.
4. **Public chunk transactions** (`Session::transact`-like, with one budget across calls), so that a chunk
   is atomic and `--stmt-timeout` rolls back the whole chunk, as it rolls back one statement today.

**Lua 5.4 against Luau.** A scratch crate outside the workspace, mlua 0.12.2, release profile, macOS arm64:

| | Lua 5.4 (`lua54`, `vendored`) | Luau (`luau`) |
|---|---|---|
| Native toolchain | C (`cc`), so it fits the `cc`-only container | C++17 (`cc` with `cpp(true)`), so the container needs a C++ compiler |
| Clean release build of the test crate | 14.8 s | 33.9 s |
| Binary of the test crate | 0.92 MB | 2.21 MB |
| VM loop, 1e7 iterations | 69 ms | 87 ms |
| 1e6 calls into Rust | 37 ms | 40 ms |
| 2e5 `__index` metamethod hits (the proxies' hot path) | 37 ms | 73 ms |
| Number model | integer and float subtypes (Python's `int`/`float`) | doubles only (mlua turns integral doubles into integers, so `2.0` arrives as `int`) |
| Sandbox | built by hand (§3): allow-list, `load` wrapper, locked metatables | `Lua::sandbox(true)`: read-only globals and standard library, no bytecode `load`, a restricted `os`, no `io` |
| Stopping runaway code | instruction hook (`set_global_hook`) plus wrapped `pcall` | `set_interrupt` (function calls and loop back-edges) plus wrapped `pcall` |
| C functions interruptible | no | no |
| Syntax for agents | plain Lua | adds `+=`, `continue`, string interpolation and type annotations |
| `while true do end` stopped after a 100 ms deadline | yes, after 100 ms | yes, after 100 ms |

## 7. Decision: no Lua front-end in `frap-cli` for now

Arguments against shipping:

- **frap's users are LLM agents, and they already speak bpy.** frap's whole point is Blender-identical
  terminology. Agents write `bpy.ops.mesh.bevel(offset=0.1)` from training data. The Lua mapping (§2) adds
  rules they would have to learn: a trailing keyword table, `None` as a sentinel, `Set{…}`, `:` against `.`,
  and above all 0-based proxies next to 1-based value tables. Each rule is a way to get a call wrong, and
  that call would never fail the same way in Blender.
- **What Lua adds is control flow, and agents rarely need it in-process.** Variables, loops and conditionals
  are what the bpy-line grammar leaves out on purpose (§11.3). An agent gets the same effect by generating
  the statements, or by reading a value and deciding on the next request. The demo `for i = 0, 4 do
  primitive_cube_add{…}` is five lines of bpy-line, and each line gets its own response.
- **The cost is permanent.** A second grammar doubles the documentation (agents.md, `--describe` tool
  schemas), needs its own goldens, and a sandbox that has to be maintained and audited. It also brings C
  into a binary whose default build is otherwise C-free for this layer. The open gaps (§3: non-transactional
  chunks, C functions that cannot be interrupted) would need §6's API changes before the front-end would
  meet the guarantees the Python form already gives (one statement = one transaction, rollback on timeout).

If an agent workload does need in-process control flow later, **use Lua 5.4, not Luau**:

- the integer/float subtypes keep Blender-exact coercion errors identical across the two front-ends, which
  Luau cannot express;
- C only, so it builds in the `cc`-only container;
- smaller and quicker to build;
- twice as fast on the metamethod path the proxies live on.

Luau's built-in sandbox is its real advantage. The spike shows that an allow-list, a text-only `load`,
locked metatables and wrapped `pcall` give the same guarantees for this use, with the same remaining gap
(C functions). Ship it only as `frap --lua` / `serve` with `"lang": "lua"`, after the §6 Session API changes
and a §2 table in `docs/agents.md`.

**Disposition.** Keep `crates/frap-lua` behind `lua`. CI's `optional-codecs` job keeps it compiling
(clippy). Its tests run with `cargo test -p frap-lua --features lua`. No deviation row and no OPERATORS.md
row: the spike changes no Blender-visible behaviour.
