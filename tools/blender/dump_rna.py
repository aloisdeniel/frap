# SPDX-License-Identifier: GPL-3.0-or-later
"""Dump Blender RNA struct definitions (bl_rna) to JSON, one file per struct.

Runs INSIDE Blender (ARCHITECTURE.md §4.1):

    $BLENDER_BIN -b --factory-startup --python tools/blender/dump_rna.py -- \
        --out spec/blender-5.2/ [--structs Object,Mesh,...]

Writes <out>/rna/<Struct>.json and <out>/rna/_meta.json.

Struct set ("every struct reachable from bpy.types that frap may implement"): the ROOTS below
(OPERATORS.md §D, every Modifier subclass, every ShaderNode*/NodeSocket* type, the glTF add-on
preferences), closed over (a) base structs, (b) the fixed types and collection API structs of
their POINTER/COLLECTION properties, never entering an ID type that is not itself a root.

For every struct: identifier, name, description, base, name_property, nested, is_id,
own properties (base-chain properties are NOT repeated; lookup walks `base`),
functions (params / returns), and `fresh`: values read from a newly created
instance wherever they differ from the RNA default (ARCHITECTURE.md §2.1, §5.6, RFC 0001
F-DEF-7), with the creation recipe.

The struct `Context` additionally gets `context_members`: members reachable via
`bpy.context.<name>` that are NOT RNA properties of `bpy.types.Context` (they come
from C context callbacks: object, active_object, selected_objects, ...). Their types
are inferred from live values inside a VIEW_3D override and from a few scene states;
`spec/overlay/context_members.toml` completes the ones never seen non-None (F-DEF-5).

The glTF add-on preference `KHR_materials_variants_ui` is switched on first (§4.1), so the
variant RNA (`Scene.gltf2_KHR_materials_variants_variants`, ...) is registered and dumped;
`_meta.json` records it under `addon_prefs`.

First version written for WP 0.2 (contract probes); extended to the full struct set by WP 1.1.
"""

import json
import math
import os
import sys

import bpy

# OPERATORS.md §D plus what the bindings and generated plain structs need.
ROOTS = """
ID Library Window AddonPreferences BlendData Context Preferences PreferencesEdit
Object ObjectModifiers Modifier
Mesh MeshVertex MeshEdge MeshPolygon MeshLoop MeshLoopTriangle
AttributeGroupMesh Attribute FloatAttribute FloatVectorAttribute FloatColorAttribute
ByteColorAttribute BoolAttribute IntAttribute Float2Attribute ByteIntAttribute
StringAttribute QuaternionAttribute Float4x4Attribute Short2Attribute Int2Attribute
MeshUVLoopLayer MeshUVLoop UVLoopLayers
Material MaterialSlot IDMaterials PointCloud
NodeTree ShaderNodeTree GeometryNodeTree Node NodeSocket NodeLink Nodes NodeLinks
NodeGroupInput NodeGroupOutput NodeFrame NodeReroute NodeTreeInterface
Camera CameraDOFSettings Light PointLight SunLight SpotLight AreaLight World
Scene RenderSettings ImageFormatSettings ColorManagedViewSettings ColorManagedDisplaySettings
ColorManagedInputColorspaceSettings SceneDisplay View3DShading SceneEEVEE ToolSettings
View3DCursor UnitSettings ViewLayer LayerObjects LayerCollection
Collection CollectionObjects CollectionChildren Depsgraph DepsgraphObjectInstance
Image ImagePackedFile Texture ImageTexture
VertexGroup VertexGroups VertexGroupElement Key ShapeKey
Armature Bone EditBone ArmatureEditBones Pose PoseBone BoneCollection
Action ActionSlot ActionLayer ActionStrip ActionKeyframeStrip ActionChannelbag FCurve Keyframe
FCurveKeyframePoints ActionGroup AnimData NlaTrack NlaTracks NlaStrip NlaStrips
Constraint TimelineMarker
""".split()

FLAG_ATTRS = [
    # (bpy attribute, flag name in the dump)
    ("is_readonly", "READONLY"),
    ("is_hidden", "HIDDEN"),
    ("is_skip_save", "SKIP_SAVE"),
    ("is_enum_flag", "ENUM_FLAG"),
    ("is_required", "REQUIRED"),
    ("is_output", "OUTPUT"),
    ("is_animatable", "ANIMATABLE"),
    ("is_never_none", "NEVER_NULL"),
    ("is_argument_optional", "ARGUMENT_OPTIONAL"),
    # `is_overridable` / `is_library_editable` are evaluated on a NULL instance and crash
    # Blender for some dynamic arrays (ImagePreview.image_pixels); frap needs neither.
    ("is_skip_preset", "SKIP_PRESET"),
    ("is_path_output", "PATH_OUTPUT"),
    ("is_path_supports_blend_relative", "PATH_SUPPORTS_BLEND_RELATIVE"),
    ("is_path_supports_templates", "PATH_SUPPORTS_TEMPLATES"),
    ("is_deprecated", "DEPRECATED"),
    ("is_runtime", "RUNTIME"),
    ("is_registered", "REGISTERED"),
    ("is_registered_optional", "REGISTERED_OPTIONAL"),
]


# ----------------------------------------------------------------------------- helpers

def jfloat(x):
    """JSON-safe float. Non-finite values use the protocol's tagged form (§13.3)."""
    x = float(x)
    if math.isnan(x):
        return {"$float": "nan"}
    if math.isinf(x):
        return {"$float": "inf" if x > 0 else "-inf"}
    return x


def jvalue(v):
    """Convert a live Python/mathutils/bpy value to JSON (for fresh-instance values)."""
    if v is None or isinstance(v, (bool, int, str)):
        return v
    if isinstance(v, float):
        return jfloat(v)
    if isinstance(v, (set, frozenset)):
        return sorted(v)
    if isinstance(v, bpy.types.ID):
        return {"$ref": repr(v)}
    if isinstance(v, bpy.types.bpy_struct):
        return {"$ptr": repr(v), "type": v.bl_rna.identifier}
    if hasattr(v, "__len__") and hasattr(v, "__getitem__"):
        try:
            return [jvalue(x) for x in v]
        except Exception:
            pass
    return {"$repr": repr(v)}


def array_shape(p):
    """[] for scalars, [n] for 1-D, [r, c] for 2-D (bound_box (8,3), matrix_world (4,4))."""
    if p.type not in ("BOOLEAN", "INT", "FLOAT"):
        return []
    dims = [d for d in p.array_dimensions if d]
    if not getattr(p, "is_array", False) and not dims:
        return []
    if getattr(p, "is_array", False) and not dims:
        return ["dynamic"]
    return dims


def enum_items(items):
    out = []
    for it in items:
        out.append({
            "identifier": it.identifier,
            "name": it.name,
            "description": it.description,
            "icon": it.icon,
            "value": it.value,
        })
    return out


def prop_to_json(p):
    d = {
        "identifier": p.identifier,
        "name": p.name,
        "description": p.description,
        "type": p.type,
        "subtype": p.subtype,
        "unit": p.unit,
    }
    shape = array_shape(p)
    if shape:
        d["array"] = shape
    if p.type in ("BOOLEAN", "INT", "FLOAT"):
        if shape:
            dflt = list(p.default_array)
            d["default"] = [jfloat(x) if p.type == "FLOAT" else x for x in dflt]
        else:
            d["default"] = jfloat(p.default) if p.type == "FLOAT" else p.default
        if p.type in ("INT", "FLOAT"):
            conv = jfloat if p.type == "FLOAT" else int
            d["hard_min"] = conv(p.hard_min)
            d["hard_max"] = conv(p.hard_max)
            d["soft_min"] = conv(p.soft_min)
            d["soft_max"] = conv(p.soft_max)
            d["step"] = jfloat(p.step) if p.type == "FLOAT" else p.step
        if p.type == "FLOAT":
            d["precision"] = p.precision
    elif p.type == "STRING":
        d["default"] = p.default
        d["length_max"] = p.length_max          # includes the NUL byte; 0 = unlimited
    elif p.type == "ENUM":
        d["items"] = enum_items(p.enum_items)
        static = enum_items(p.enum_items_static)
        if [i["identifier"] for i in static] != [i["identifier"] for i in d["items"]]:
            d["items_static"] = static
        if p.is_enum_flag:
            d["default"] = sorted(p.default_flag)
        else:
            d["default"] = p.default
    elif p.type == "POINTER":
        d["fixed_type"] = p.fixed_type.identifier if p.fixed_type else None
        d["is_id"] = bool(p.fixed_type and is_id_struct(p.fixed_type))
    elif p.type == "COLLECTION":
        d["fixed_type"] = p.fixed_type.identifier if p.fixed_type else None
        d["is_id"] = bool(p.fixed_type and is_id_struct(p.fixed_type))
        d["api"] = p.srna.identifier if p.srna else None
    d["flags"] = [name for attr, name in FLAG_ATTRS if getattr(p, attr, False)]
    if getattr(p, "is_deprecated", False):
        d["deprecated"] = {
            "note": p.deprecated_note,
            "version": list(p.deprecated_version),
            "removal_version": list(p.deprecated_removal_version),
        }
    tags = sorted(getattr(p, "tags", set()) or [])
    if tags:
        d["tags"] = tags
    return d


def is_id_struct(srna):
    s = srna
    while s is not None:
        if s.identifier == "ID":
            return True
        s = s.base
    return False


def func_to_json(f):
    params, returns = [], []
    for p in f.parameters:
        (returns if p.is_output else params).append(prop_to_json(p))
    return {
        "identifier": f.identifier,
        "description": f.description,
        "use_self": f.use_self,
        "use_self_type": f.use_self_type,
        "is_registered": f.is_registered,
        "is_registered_optional": f.is_registered_optional,
        "params": params,
        "returns": returns,
    }


def own_props(srna):
    base_ids = set(srna.base.properties.keys()) if srna.base else set()
    return [p for p in srna.properties if p.identifier != "rna_type" and p.identifier not in base_ids]


def own_funcs(srna):
    base_ids = set(srna.base.functions.keys()) if srna.base else set()
    return [f for f in srna.functions if f.identifier not in base_ids]


def struct_to_json(srna):
    return {
        "identifier": srna.identifier,
        "name": srna.name,
        "description": srna.description,
        "base": srna.base.identifier if srna.base else None,
        "name_property": srna.name_property.identifier if srna.name_property else None,
        "nested": srna.nested.identifier if srna.nested else None,
        "is_id": is_id_struct(srna),
        "props": [prop_to_json(p) for p in own_props(srna)],
        "funcs": [func_to_json(f) for f in own_funcs(srna)],
    }


# ------------------------------------------------------------------------- struct set

# Structs not exposed in bpy.types (the glTF add-on preferences class): identifier -> getter.
# A getter, not the struct: `read_factory_settings` re-registers add-ons and frees the old one.
_EXTRA_RNA = {}


def _gltf_prefs_rna():
    return type(bpy.context.preferences.addons["io_scene_gltf2"].preferences).bl_rna


def bl_rna_of(name):
    if name in _EXTRA_RNA:
        return _EXTRA_RNA[name]()
    t = getattr(bpy.types, name, None)
    return getattr(t, "bl_rna", None) if t is not None else None


def subclasses_of(base_name):
    """Every bpy.types struct whose base chain contains `base_name` (C types have no
    Python __subclasses__)."""
    out = []
    for n in dir(bpy.types):
        s = bl_rna_of(n)
        if s is None or s.identifier != n:
            continue
        b = s.base
        while b is not None:
            if b.identifier == base_name:
                out.append(n)
                break
            b = b.base
    return sorted(out)


def gltf_addon_prefs_struct():
    try:
        srna = _gltf_prefs_rna()
    except Exception:  # noqa: BLE001
        return None
    _EXTRA_RNA[srna.identifier] = _gltf_prefs_rna
    return srna.identifier


def root_set():
    roots = list(ROOTS)
    roots += subclasses_of("Modifier")
    roots += [n for n in subclasses_of("ShaderNode")]
    roots += [n for n in dir(bpy.types) if n.startswith("NodeSocket") and bl_rna_of(n) is not None]
    roots += subclasses_of("Light")
    p = gltf_addon_prefs_struct()
    if p:
        roots.append(p)
    out = []
    for n in roots:
        s = bl_rna_of(n)
        if s is None:
            print(f"dump_rna.py: warning: no struct {n}", file=sys.stderr)
            continue
        if s.identifier not in out:
            out.append(s.identifier)
    return out


# The closure does not enter UI, editor, tool, physics and other out-of-scope structs (their
# POINTER/COLLECTION props stay in the dump with the target identifier). Roots are kept.
PRUNE_PREFIXES = (
    "Theme", "UI", "Gizmo", "KeyMap", "KeyConfig", "Macro", "Event", "NDOF", "Xr", "Area",
    "Region", "Space", "View2D", "View3DOverlay", "Preferences", "Strip", "Sequence", "Freestyle",
    "Lineset", "FileSelect", "Walk", "Textbox", "ScriptDirectory", "UserExtension",
    "UserAssetLibrary", "UserSolidLight", "AssetLibrary", "StudioLight", "Operator", "Particle",
    "ChildParticle", "Fluid", "DynamicPaint", "Cloth", "SoftBody", "PointCache", "RigidBody",
    "Movie", "GPencil", "Gp", "GreasePencil", "Sculpt", "Paint", "VertexPaint", "ImagePaint",
    "UvSculpt", "CurvesSculpt", "UnifiedPaint", "MeshAutomasking", "IDOverrideLibrary",
    "KeyingSet", "Driver", "ChannelDriver", "AnimDataDrivers", "MotionPath", "AnimViz",
    "Lightgroup", "AOV", "Stereo3d", "BakeSettings", "FFmpeg", "RenderView", "SceneRenderView",
    "SceneHydra", "Struct", "Property", "Function", "EnumPropertyItem", "StringProperty",
    "CollisionSettings", "EffectorWeights", "FieldSettings", "CurvePaintSettings",
    "MeshStatVis", "TransformOrientation", "CurveProfile", "SequencerToolSettings",
    "MaterialGPencilStyle", "MaterialLineArt", "ObjectLineArt", "ObjectShaderFx", "ShaderFx",
    "DisplaySafeAreas", "ActionPoseMarkers", "LibraryWeakReference",
    "PathCompare", "SceneGpencil", "RaytraceEEVEE", "WorldMistSettings", "WorldLighting",
    "BlendFileColorspace", "TexPaintSlot", "NodeInternalSocketTemplate", "Asset",
    "IKParam", "StripModifier",
)
# Exceptions to PRUNE_PREFIXES that frap needs (the virtual 3D View of §8).
PRUNE_KEEP = {"RegionView3D", "Addon", "Addons", "AddonPreferences"}


def pruned(name):
    return name not in PRUNE_KEEP and name.startswith(PRUNE_PREFIXES)


def closure(roots):
    """Roots + bases + POINTER/COLLECTION targets and collection API structs, never entering an
    ID type that is not a root nor a pruned struct. Sorted for determinism."""
    root_set_ = set(roots)
    seen = set(roots)
    frontier = list(roots)
    while frontier:
        nxt = []
        for n in frontier:
            s = bl_rna_of(n)
            b = s.base
            while b is not None:
                if b.identifier not in seen:
                    seen.add(b.identifier)
                    nxt.append(b.identifier)
                b = b.base
            for p in list(s.properties) + [q for f in s.functions for q in f.parameters]:
                if p.type not in ("POINTER", "COLLECTION") or p.identifier == "rna_type":
                    continue
                for t in (p.fixed_type, getattr(p, "srna", None)):
                    if t is None or t.identifier in seen or t.identifier in root_set_:
                        continue
                    if is_id_struct(t) or pruned(t.identifier):
                        continue
                    if bl_rna_of(t.identifier) is None:
                        # M7 W0: PropertyGroups an add-on registers under another class name
                        # (the glTF variant structs) are not in `bpy.types` by identifier;
                        # re-resolve them through the owning property each time (fresh
                        # instances reload factory settings, which frees the old struct).
                        # Only the glTF add-on's (Cycles' and other add-ons' stay out).
                        if (
                            not t.identifier.startswith("gltf2_")
                            or p.identifier not in s.properties
                            or t != p.fixed_type
                        ):
                            continue
                        _EXTRA_RNA[t.identifier] = (
                            lambda o=n, q=p.identifier: bl_rna_of(o).properties[q].fixed_type
                        )
                    seen.add(t.identifier)
                    nxt.append(t.identifier)
        frontier = nxt
    return sorted(seen)


# ----------------------------------------------------------------- fresh-instance values

def _principled(mat):
    return mat.node_tree.nodes["Principled BSDF"]


def fresh_instances():
    """Recipes creating a fresh instance per struct (the creation path is recorded).
    Each returns (recipe_text, instance)."""
    D = bpy.data

    def new_scene():
        return D.scenes.new("FreshScene")

    def scene_attr(path):
        def f():
            sc = new_scene()
            v = sc
            for a in path.split("."):
                v = getattr(v, a)
            return f"bpy.data.scenes.new('Scene').{path}", v
        return f

    def light(t, cls):
        def f():
            return f"bpy.data.lights.new('Light', '{t}')", D.lights.new("Fresh" + cls, t)
        return f

    def obj():
        return "bpy.data.objects.new('Object', None)", D.objects.new("Object", None)

    def mesh():
        return "bpy.data.meshes.new('Mesh')", D.meshes.new("Mesh")

    def mat():
        return "bpy.data.materials.new('Material')", D.materials.new("Material")

    def ntree():
        m = D.materials.new("FreshNT")
        return "bpy.data.materials.new('Material').node_tree", m.node_tree

    def vert():
        me = D.meshes.new("FreshVert")
        me.vertices.add(1)
        return "me = bpy.data.meshes.new('Mesh'); me.vertices.add(1); me.vertices[0]", me.vertices[0]

    def node():
        m = D.materials.new("FreshNode")
        return "bpy.data.materials.new('Material').node_tree.nodes['Principled BSDF']", _principled(m)

    def sock():
        m = D.materials.new("FreshSock")
        return ("bpy.data.materials.new('Material').node_tree.nodes['Principled BSDF'].inputs[0]",
                _principled(m).inputs[0])

    def slot():
        me = D.meshes.new("FreshSlot")
        me.materials.append(D.materials.new("FreshSlotMat"))
        ob = D.objects.new("FreshSlot", me)
        return ("me = bpy.data.meshes.new('Mesh'); me.materials.append(bpy.data.materials.new('FreshSlotMat')); "
                "bpy.data.objects.new('O', me).material_slots[0]"), ob.material_slots[0]

    def camera():
        return "bpy.data.cameras.new('Camera')", D.cameras.new("FreshCamera")

    def dof():
        return "bpy.data.cameras.new('Camera').dof", D.cameras.new("FreshDof").dof

    def world():
        return "bpy.data.worlds.new('World')", D.worlds.new("FreshWorld")

    def coll():
        return "bpy.data.collections.new('Collection')", D.collections.new("FreshCollection")

    def image():
        return "bpy.data.images.new('Image', 4, 4)", D.images.new("FreshImage", 4, 4)

    def image_cs():
        im = D.images.new("FreshImageCS", 4, 4)
        return "bpy.data.images.new('Image', 4, 4).colorspace_settings", im.colorspace_settings

    def scene():
        return "bpy.data.scenes.new('Scene')", new_scene()

    def pointcloud():
        return "bpy.data.pointclouds.new('PointCloud')", D.pointclouds.new("FreshPointCloud")

    def armature():
        return "bpy.data.armatures.new('Armature')", D.armatures.new("FreshArmature")

    def action():
        return "bpy.data.actions.new('Action')", D.actions.new("FreshAction")

    return {
        "Object": obj, "Mesh": mesh, "Material": mat, "ShaderNodeTree": ntree,
        "MeshVertex": vert, "Node": node, "ShaderNodeBsdfPrincipled": node,
        "NodeSocket": sock, "NodeSocketColor": sock, "MaterialSlot": slot,
        "Camera": camera, "CameraDOFSettings": dof,
        "Light": light("POINT", "Light"), "PointLight": light("POINT", "Point"),
        "SunLight": light("SUN", "Sun"), "SpotLight": light("SPOT", "Spot"),
        "AreaLight": light("AREA", "Area"),
        "World": world, "Collection": coll, "Image": image,
        "ColorManagedInputColorspaceSettings": image_cs,
        "Scene": scene,
        "RenderSettings": scene_attr("render"),
        "ImageFormatSettings": scene_attr("render.image_settings"),
        "ColorManagedViewSettings": scene_attr("view_settings"),
        "ColorManagedDisplaySettings": scene_attr("display_settings"),
        "SceneDisplay": scene_attr("display"),
        "View3DShading": scene_attr("display.shading"),
        "SceneEEVEE": scene_attr("eevee"),
        "ToolSettings": scene_attr("tool_settings"),
        "UnitSettings": scene_attr("unit_settings"),
        "View3DCursor": scene_attr("cursor"),
        "PointCloud": pointcloud, "Armature": armature, "Action": action,
    }


def rna_default_json(p):
    if p.type in ("BOOLEAN", "INT", "FLOAT"):
        if array_shape(p):
            return [jfloat(x) if p.type == "FLOAT" else x for x in p.default_array]
        return jfloat(p.default) if p.type == "FLOAT" else p.default
    if p.type == "STRING":
        return p.default
    if p.type == "ENUM":
        return sorted(p.default_flag) if p.is_enum_flag else p.default
    return None


def flatten(v):
    if isinstance(v, list):
        out = []
        for x in v:
            out.extend(flatten(x))
        return out
    return [v]


# Per-instance identity, not a "fresh value" (would make every dump differ run to run).
IDENTITY_PROPS = {"rna_type", "session_uid", "bindcode"}
# ID names are given by the recipe; a non-ID struct's `name` is a value (colour space name).
ID_IDENTITY_PROPS = {"name", "name_full"}
# Machine-dependent fresh values (frap is deterministic and keeps the RNA default):
# `RenderSettings.threads` is the CPU thread count of the dumping machine.
MACHINE_PROPS = {("RenderSettings", "threads")}


def fresh_diff(srna, inst):
    """Values on the fresh instance that differ from the RNA default (scalars/arrays/enums/strings)."""
    diff = {}
    for p in srna.properties:
        if p.identifier in IDENTITY_PROPS or p.type in ("POINTER", "COLLECTION"):
            continue
        if p.identifier in ID_IDENTITY_PROPS and is_id_struct(srna):
            continue
        if (srna.identifier, p.identifier) in MACHINE_PROPS:
            continue
        try:
            live = jvalue(getattr(inst, p.identifier))
        except Exception as e:  # noqa: BLE001 - recorded, not fatal
            diff[p.identifier] = {"$error": f"{type(e).__name__}: {e}"}
            continue
        dflt = rna_default_json(p)
        if isinstance(live, list) and isinstance(dflt, list):
            if flatten(live) != flatten(dflt):
                diff[p.identifier] = live
        elif live != dflt:
            diff[p.identifier] = live
    return diff


# ------------------------------------------------------------------ context members

def context_members():
    """Members of bpy.context that are not RNA properties of bpy.types.Context.
    Types are inferred from live values in several states (inside a VIEW_3D override)."""
    rna_props = set(bpy.types.Context.bl_rna.properties.keys())
    seen = {}

    def classify(name, v):
        if name in rna_props or name.startswith("_"):
            return
        entry = seen.setdefault(name, {"identifier": name, "kinds": set(), "types": set()})
        if v is None:
            entry["kinds"].add("pointer_or_none")
        elif isinstance(v, (list, tuple)):
            entry["kinds"].add("collection")
            for x in v:
                entry["types"].add(type(x).bl_rna.identifier if hasattr(type(x), "bl_rna") else type(x).__name__)
        elif isinstance(v, bpy.types.bpy_struct):
            entry["kinds"].add("pointer")
            entry["types"].add(v.bl_rna.identifier)
        elif isinstance(v, str):
            entry["kinds"].add("string")
        else:
            entry["kinds"].add(type(v).__name__)

    def snapshot():
        win = bpy.context.window
        area = next(a for a in win.screen.areas if a.type == "VIEW_3D")
        region = next(r for r in area.regions if r.type == "WINDOW")
        with bpy.context.temp_override(window=win, area=area, region=region):
            for k, v in bpy.context.copy().items():
                classify(k, v)

    snapshot()                                  # factory: Cube active + selected
    bpy.ops.object.armature_add()
    bpy.ops.object.mode_set(mode="POSE")
    snapshot()                                  # pose mode
    bpy.ops.object.mode_set(mode="EDIT")
    snapshot()                                  # armature edit mode
    bpy.ops.object.mode_set(mode="OBJECT")
    bpy.ops.wm.read_factory_settings()
    bpy.ops.object.mode_set(mode="EDIT")
    snapshot()                                  # mesh edit mode
    bpy.ops.wm.read_factory_settings()

    out = []
    for name in sorted(seen):
        e = seen[name]
        kinds = e["kinds"] - {"pointer_or_none"}
        out.append({
            "identifier": name,
            "kind": "collection" if "collection" in kinds else ("pointer" if "pointer" in kinds else
                     (sorted(kinds)[0] if kinds else "unknown")),
            "types": sorted(e["types"]),
            "seen_none": "pointer_or_none" in e["kinds"],
        })
    return out


# ----------------------------------------------------------------------------- main

def parse_args():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    args = {"out": None, "structs": None}
    i = 0
    while i < len(argv):
        a = argv[i]
        if a == "--out":
            args["out"] = argv[i + 1]; i += 2
        elif a == "--structs":
            args["structs"] = [s for s in argv[i + 1].split(",") if s]; i += 2
        else:
            raise SystemExit(f"dump_rna.py: unknown argument {a!r}")
    if not args["out"]:
        raise SystemExit("dump_rna.py: --out DIR is required")
    return args


ADDON_PREFS = {"io_scene_gltf2": {"KHR_materials_variants_ui": True}}


def enable_addon_prefs():
    # §4.1: without this, glTF variant RNA is not registered.
    for addon, values in ADDON_PREFS.items():
        for k, v in values.items():
            try:
                setattr(bpy.context.preferences.addons[addon].preferences, k, v)
            except Exception as e:  # noqa: BLE001
                raise SystemExit(f"dump_rna.py: cannot set {addon}.{k}: {e}")


def addon_prefs_state():
    out = {}
    for addon, values in ADDON_PREFS.items():
        prefs = bpy.context.preferences.addons[addon].preferences
        out[addon] = {k: getattr(prefs, k) for k in values}
    return out


def blender_meta():
    return {
        "blender_version": bpy.app.version_string,
        "build_hash": bpy.app.build_hash.decode() if isinstance(bpy.app.build_hash, bytes) else bpy.app.build_hash,
        "factory_startup": True,
        "addon_prefs": addon_prefs_state(),
        "property_types": [i.identifier for i in bpy.types.Property.bl_rna.properties["type"].enum_items],
        "subtypes": [i.identifier for i in bpy.types.Property.bl_rna.properties["subtype"].enum_items],
        "units": [i.identifier for i in bpy.types.Property.bl_rna.properties["unit"].enum_items],
    }


def write_json(path, data):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=1, ensure_ascii=False, allow_nan=False, sort_keys=False)
        f.write("\n")


def main():
    args = parse_args()
    enable_addon_prefs()
    roots = args["structs"] or root_set()
    names = closure(roots)

    # Context members first (it reloads factory settings, which would free fresh instances).
    ctx_members = context_members()
    enable_addon_prefs()

    recipes = fresh_instances()
    out_dir = os.path.join(args["out"], "rna")
    os.makedirs(out_dir, exist_ok=True)
    for f in os.listdir(out_dir):
        if f.endswith(".json"):
            os.remove(os.path.join(out_dir, f))
    for n in names:
        srna = bl_rna_of(n)
        data = struct_to_json(srna)
        data["root"] = n in roots
        if n in recipes:
            recipe, inst = recipes[n]()
            data["fresh"] = {"recipe": recipe, "differs_from_default": fresh_diff(srna, inst)}
        if n == "Context":
            data["context_members"] = ctx_members
        write_json(os.path.join(out_dir, n + ".json"), data)
    meta = blender_meta()
    meta["roots"] = sorted(roots)
    meta["structs"] = names
    write_json(os.path.join(out_dir, "_meta.json"), meta)
    print(f"dump_rna.py: {len(names)} structs ({len(roots)} roots)")


if __name__ == "__main__":
    main()
