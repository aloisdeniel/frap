# SPDX-License-Identifier: GPL-3.0-or-later
"""Blender side of `crates/frap-anim/tests/blender_nla.rs` (WP 6.1): NLA blending.

Each scenario animates the default Cube through actions placed in NLA tracks (blend types,
influence and blend in/out, extrapolation, scale, repeat, reverse, solo, mute, the action track
on top), evaluates the scene at whole and fractional frames and dumps the animated channels'
values, with the actions (keys and handles) and the AnimData / track / strip settings Blender
ended up with. The Rust test rebuilds the same data and compares `evaluate_anim`.

    Blender -b --factory-startup --python crates/frap-anim/tests/blender/dump_nla.py -- \
        crates/frap-anim/tests/fixtures/blender_nla.json
"""

import json
import sys

import bpy

CHANNELS = [("location", 3), ("scale", 3), ("rotation_quaternion", 4),
            ("rotation_axis_angle", 4), ("rotation_euler", 3)]


def make_action(name, curves):
    """curves: {(path, index): [(frame, value, ipo), ...]}"""
    act = bpy.data.actions.new(name)
    slot = act.slots.new('OBJECT', "Cube")
    bag = act.layers.new("Layer").strips.new(type='KEYFRAME').channelbag(slot, ensure=True)
    for (path, index), keys in curves.items():
        fc = bag.fcurves.new(path, index=index)
        for f, v, ipo in keys:
            k = fc.keyframe_points.insert(f, v)
            k.interpolation = ipo
        fc.update()
    return act


def reset(ob):
    ob.animation_data_clear()
    ob.rotation_mode = 'XYZ'
    ob.location = (0, 0, 0)
    ob.scale = (1, 1, 1)
    ob.rotation_euler = (0, 0, 0)
    ob.rotation_quaternion = (1, 0, 0, 0)
    ob.rotation_axis_angle = (0, 0, 1, 0)


def dump_action(act):
    slots = [{"handle": s.handle, "identifier": s.identifier, "target_id_type": s.target_id_type}
             for s in act.slots]
    bags = []
    for bag in act.layers[0].strips[0].channelbags:
        bags.append({"slot_handle": bag.slot_handle, "fcurves": [{
            "data_path": fc.data_path, "array_index": fc.array_index,
            "extrapolation": fc.extrapolation,
            "keys": [{"co": list(k.co), "handle_left": list(k.handle_left),
                      "handle_right": list(k.handle_right), "interpolation": k.interpolation,
                      "easing": k.easing} for k in fc.keyframe_points],
        } for fc in bag.fcurves]})
    return {"name": act.name, "slots": slots, "channelbags": bags}


STRIP_PROPS = ["name", "type", "extrapolation", "blend_type", "frame_start", "frame_end",
               "action_frame_start", "action_frame_end", "scale", "repeat", "blend_in",
               "blend_out", "influence", "use_animated_influence", "strip_time",
               "use_animated_time", "mute", "use_reverse", "action_slot_handle"]


def dump_adt(adt):
    return {
        "action": adt.action.name if adt.action else None,
        "action_slot_handle": adt.action_slot_handle,
        "use_nla": adt.use_nla,
        "action_influence": adt.action_influence,
        "action_blend_type": adt.action_blend_type,
        "action_extrapolation": adt.action_extrapolation,
        "tracks": [{
            "name": t.name, "mute": t.mute, "is_solo": t.is_solo,
            "strips": [dict({p: getattr(s, p) for p in STRIP_PROPS},
                            action=s.action.name if s.action else None) for s in t.strips],
        } for t in adt.nla_tracks],
    }


def scenarios():
    loc_up = {("location", 0): [(1, 0.0, 'LINEAR'), (11, 10.0, 'LINEAR')],
              ("location", 2): [(1, 1.0, 'BEZIER'), (6, 3.0, 'BEZIER'), (11, 1.0, 'BEZIER')]}
    loc_side = {("location", 1): [(1, -2.0, 'BEZIER'), (21, 4.0, 'BEZIER')],
                ("location", 0): [(1, 5.0, 'CONSTANT'), (10, 7.0, 'CONSTANT')]}
    scl = {("scale", 0): [(1, 2.0, 'LINEAR'), (11, 0.5, 'LINEAR')],
           ("scale", 2): [(1, 1.5, 'LINEAR'), (11, 3.0, 'LINEAR')]}
    quat = {("rotation_quaternion", 0): [(1, 1.0, 'LINEAR'), (11, 0.7071068, 'LINEAR')],
            ("rotation_quaternion", 3): [(1, 0.0, 'LINEAR'), (11, 0.7071068, 'LINEAR')]}
    quat2 = {("rotation_quaternion", 0): [(1, 0.7071068, 'LINEAR')],
             ("rotation_quaternion", 1): [(1, 0.7071068, 'LINEAR')]}
    axis = {("rotation_axis_angle", 0): [(1, 0.0, 'LINEAR'), (11, 1.5, 'LINEAR')],
            ("rotation_axis_angle", 3): [(1, 1.0, 'LINEAR')]}

    def one(ob, actions):
        return {k: make_action(f"{k}", v) for k, v in actions.items()}

    out = []

    def add(name, build):
        ob = bpy.data.objects['Cube']
        reset(ob)
        for a in list(bpy.data.actions):
            bpy.data.actions.remove(a)
        build(ob)
        out.append(evaluate(name, ob))

    def s1(ob):
        a = one(ob, {"Up": loc_up})
        adt = ob.animation_data_create()
        t = adt.nla_tracks.new()
        s = t.strips.new("S", 5, a["Up"])
        s.blend_in = 3.0
        s.blend_out = 2.0
        s.extrapolation = 'HOLD'
    add("replace_blend_in_out", s1)

    def s2(ob):
        a = one(ob, {"Up": loc_up, "Side": loc_side, "Scl": scl})
        adt = ob.animation_data_create()
        t1 = adt.nla_tracks.new()
        t1.strips.new("Base", 1, a["Up"])
        for i, mode in enumerate(["ADD", "SUBTRACT", "MULTIPLY", "REPLACE"]):
            t = adt.nla_tracks.new()
            s = t.strips.new(mode, 3 + i, a["Side"] if mode != "MULTIPLY" else a["Scl"])
            s.blend_type = mode
            s.influence = 0.25 + 0.15 * i
            s.use_animated_influence = True
            s.extrapolation = 'HOLD_FORWARD'
    add("blend_types", s2)

    def s3(ob):
        ob.rotation_mode = 'QUATERNION'
        a = one(ob, {"Up": loc_up, "Scl": scl, "Quat": quat, "Quat2": quat2})
        adt = ob.animation_data_create()
        t1 = adt.nla_tracks.new()
        t1.strips.new("Base", 1, a["Quat"])
        t2 = adt.nla_tracks.new()
        for act, start in [(a["Up"], 1), (a["Scl"], 12)]:
            s = t2.strips.new(act.name, start, act)
            s.blend_type = 'COMBINE'
            s.influence = 0.6
            s.use_animated_influence = True
        t3 = adt.nla_tracks.new()
        s = t3.strips.new("Q2", 1, a["Quat2"])
        s.blend_type = 'COMBINE'
        s.blend_in = 4.0
        s.frame_end = 15
    add("combine", s3)

    def s3b(ob):
        ob.rotation_mode = 'AXIS_ANGLE'
        a = one(ob, {"Axis": axis, "Scl": scl})
        adt = ob.animation_data_create()
        t1 = adt.nla_tracks.new()
        t1.strips.new("Base", 1, a["Scl"])
        t2 = adt.nla_tracks.new()
        s = t2.strips.new("Axis", 2, a["Axis"])
        s.blend_type = 'COMBINE'
        s.influence = 0.5
        s.use_animated_influence = True
        t3 = adt.nla_tracks.new()
        s = t3.strips.new("Scl2", 4, a["Scl"])
        s.blend_type = 'COMBINE'
    add("combine_axis_angle_scale", s3b)

    def s4(ob):
        a = one(ob, {"Up": loc_up, "Side": loc_side})
        adt = ob.animation_data_create()
        t = adt.nla_tracks.new()
        s = t.strips.new("Scaled", 2, a["Up"])
        s.scale = 1.5
        s.repeat = 2.5
        s.extrapolation = 'HOLD_FORWARD'
        s2 = t.strips.new("Reverse", 40, a["Side"])
        s2.use_reverse = True
        s2.extrapolation = 'NOTHING'
        t2 = adt.nla_tracks.new()
        s3 = t2.strips.new("Nothing", 70, a["Up"])
        s3.extrapolation = 'NOTHING'
        s3.blend_type = 'ADD'
    add("time_mapping_and_extrapolation", s4)

    def s5(ob):
        a = one(ob, {"Up": loc_up, "Side": loc_side, "Scl": scl})
        adt = ob.animation_data_create()
        t = adt.nla_tracks.new()
        t.strips.new("Base", 1, a["Up"])
        adt.action = a["Side"]
        adt.action_blend_type = 'ADD'
        adt.action_influence = 0.5
        adt.action_extrapolation = 'HOLD_FORWARD'
    add("action_track_on_top", s5)

    def s6(ob):
        a = one(ob, {"Up": loc_up, "Side": loc_side, "Scl": scl})
        adt = ob.animation_data_create()
        t1 = adt.nla_tracks.new()
        t1.strips.new("Base", 1, a["Up"])
        t2 = adt.nla_tracks.new()
        t2.strips.new("Side", 1, a["Side"])
        t2.mute = True
        t3 = adt.nla_tracks.new()
        st = t3.strips.new("Scl", 1, a["Scl"])
        st.mute = True
        adt.action = a["Scl"]
    add("muted", s6)

    def s7(ob):
        a = one(ob, {"Up": loc_up, "Side": loc_side})
        adt = ob.animation_data_create()
        t1 = adt.nla_tracks.new()
        t1.strips.new("Base", 1, a["Up"])
        t2 = adt.nla_tracks.new()
        t2.strips.new("Side", 1, a["Side"])
        t2.is_solo = True
        adt.action = a["Up"]
    add("solo", s7)

    def s8(ob):
        a = one(ob, {"Up": loc_up, "Side": loc_side})
        adt = ob.animation_data_create()
        t1 = adt.nla_tracks.new()
        t1.strips.new("Base", 1, a["Up"])
        adt.action = a["Side"]
        adt.action_influence = 0.3
        adt.use_nla = False
    add("nla_off", s8)

    def s9(ob):
        a = one(ob, {"Up": loc_up, "Side": loc_side})
        adt = ob.animation_data_create()
        adt.nla_tracks.new()
        adt.action = a["Side"]
        adt.action_influence = 0.3
        adt.action_blend_type = 'ADD'
    add("empty_track_action_alone", s9)

    def s10(ob):
        a = one(ob, {"Up": loc_up})
        adt = ob.animation_data_create()
        t = adt.nla_tracks.new()
        s = t.strips.new("Short", 5, a["Up"])
        s.extrapolation = 'NOTHING'
        s.blend_type = 'REPLACE'
        adt.action = a["Up"]
        adt.action_extrapolation = 'NOTHING'
        adt.action_blend_type = 'COMBINE'
        adt.action_influence = 0.7
    add("domain_defaults_and_nothing", s10)
    return out


def evaluate(name, ob):
    scene = bpy.context.scene
    samples = []
    for i in range(-4, 100):
        f = i * 0.75
        whole = int(f // 1)
        scene.frame_set(whole, subframe=f - whole)
        ev = ob.evaluated_get(bpy.context.evaluated_depsgraph_get())
        samples.append([f, {p: list(getattr(ev, p)) for p, _ in CHANNELS}])
    return {"name": name, "rotation_mode": ob.rotation_mode,
            "actions": [dump_action(a) for a in bpy.data.actions],
            "animation_data": dump_adt(ob.animation_data), "samples": samples}


def main():
    out_path = sys.argv[sys.argv.index("--") + 1]
    results = scenarios()
    with open(out_path, "w") as fh:
        json.dump({"blender": bpy.app.version_string, "build_hash": bpy.app.build_hash.decode(),
                   "scenarios": results}, fh, indent=0)


main()
