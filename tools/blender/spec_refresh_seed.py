# SPDX-License-Identifier: GPL-3.0-or-later
"""Seeded spec drift for `cargo xtask spec-refresh` (ARCHITECTURE §22.10 WP 8.9).

Runs INSIDE Blender, before a dump script:

    $BLENDER_BIN -b --factory-startup --python tools/blender/spec_refresh_seed.py \
        --python tools/blender/dump_ops.py -- --out <scratch>/

`cargo xtask spec-refresh --seed tools/blender/spec_refresh_seed.py` passes it to every dump.
It changes Python-defined operators of the curated namespaces (re-registering their classes),
so a dump of the real Blender 5.2.x shows one drift of each kind the routine must report (on
operators whose OPERATORS.md rows are settled, so the expected report stays stable):

    renamed property   object.select_pattern  `case_sensitive` -> `match_case` (same position)
    new poll cell      object.select_pattern  poll() also False in Pose Mode (cells D, DV: Y -> N)
    changed default    object.subdivision_set `level` 1 -> 2
    new enum item      uv.follow_active_quads `mode` gains `UNIFORM`
    removed operator   uv.lightmap_pack       (unregistered)

The changes survive `wm.read_factory_settings()` (which the dump scripts call per context):
operator classes are not re-registered by a factory reset. The expected report is
`xtask/src/spec_refresh/fixtures/seeded.expected.md`; the hermetic test of the same report
applies these five changes to the committed dumps (`xtask/src/spec_refresh/tests.rs`).
"""

import sys

import bpy

sys.dont_write_bytecode = True


def redeferred(prop, **changes):
    """A new `_PropertyDeferred` from an existing one, with keyword changes (`attr` is set by
    registration and must not be passed back)."""
    kw = {k: v for k, v in prop.keywords.items() if k != "attr"}
    kw.update(changes)
    return prop.function(**kw)


def reregister(idname_class, annotations=None, **attrs):
    """Replaces the registered operator class `idname_class` (e.g. `OBJECT_OT_select_pattern`)
    by a copy with new annotations and class attributes."""
    old = getattr(bpy.types, idname_class)
    body = {
        k: v
        for k, v in old.__dict__.items()
        if k != "bl_rna" and (not k.startswith("__") or k == "__doc__")
    }
    body["__annotations__"] = annotations if annotations is not None else dict(old.__annotations__)
    body.update(attrs)
    bpy.utils.unregister_class(old)
    bpy.utils.register_class(type(idname_class, (bpy.types.Operator,), body))


def seed():
    # Renamed property (kept in place) and the poll additionally False in Pose Mode.
    cls = bpy.types.OBJECT_OT_select_pattern
    old_poll = cls.poll
    old = cls.__annotations__
    ann = {}
    for k, v in old.items():
        if k == "case_sensitive":
            ann["match_case"] = redeferred(v)
        else:
            ann[k] = v
    reregister(
        "OBJECT_OT_select_pattern",
        ann,
        poll=classmethod(lambda cls, context: old_poll(context) and context.mode != 'POSE'),
    )

    # Changed default.
    ann = dict(bpy.types.OBJECT_OT_subdivision_set.__annotations__)
    ann["level"] = redeferred(ann["level"], default=2)
    reregister("OBJECT_OT_subdivision_set", ann)

    # New enum item (appended).
    ann = dict(bpy.types.UV_OT_follow_active_quads.__annotations__)
    items = tuple(ann["mode"].keywords["items"]) + (
        ("UNIFORM", "Uniform", "Seeded drift (spec-refresh fixture)"),
    )
    ann["mode"] = redeferred(ann["mode"], items=items)
    reregister("UV_OT_follow_active_quads", ann)

    # Removed operator.
    bpy.utils.unregister_class(bpy.types.UV_OT_lightmap_pack)
    print("spec_refresh_seed.py: 5 seeded drifts", file=sys.stderr)


seed()
