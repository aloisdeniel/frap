import bpy, os, sys
C=bpy.context; D=bpy.data; OUT=sys.argv[-1]
def r(v): return tuple(round(x,4) for x in v)
def tryop(label, f):
    try: print(f"  {label}: {f()}")
    except Exception as e: print(f"  {label}: EXC {type(e).__name__}: {str(e).strip()[:300]}")
cube=D.objects['Cube']
bpy.ops.object.modifier_add(type='MIRROR'); bpy.ops.object.modifier_add(type='BEVEL')
tryop("modifier_remove no name", lambda: (bpy.ops.object.modifier_remove(), [m.name for m in cube.modifiers]))
tryop("modifier_apply no name", lambda: (bpy.ops.object.modifier_apply(), [m.name for m in cube.modifiers]))
bpy.ops.object.modifier_add(type='HOOK'); print("  order", [m.name for m in cube.modifiers])
tryop("move Mirror to 0 w/ hook", lambda: (bpy.ops.object.modifier_move_to_index(modifier='Mirror', index=0), [m.name for m in cube.modifiers]))
tryop("move Hook down", lambda: (bpy.ops.object.modifier_move_down(modifier='Hook'), [m.name for m in cube.modifiers]))
p=C.preferences.edit; print("  dupflags", {k:getattr(p,k) for k in dir(p) if k.startswith('use_duplicate')})
bpy.ops.object.light_add(type='POINT'); l=C.object.data; print("  point light props", {k:getattr(l,k) for k in ('energy','shadow_soft_size','use_soft_falloff','exposure','temperature') if hasattr(l,k)}, [k for k in dir(l) if 'radius' in k or 'size' in k])
bpy.ops.object.light_add(type='SPOT', radius=2.0); l=C.object.data; print("  spot radius=2", l.shadow_soft_size, l.spot_size, l.spot_blend, r(C.object.scale))
bpy.ops.object.light_add(type='AREA'); l=C.object.data; print("  area", l.shape, l.size)
bpy.ops.object.empty_add(type='CUBE', radius=3); print("  empty radius", C.object.empty_display_size, r(C.object.scale))
m=D.materials.new("M"); print("  data.materials.new nodes", m.node_tree and [n.name for n in m.node_tree.nodes])
bpy.ops.wm.read_factory_settings()
cube=D.objects['Cube']
bpy.ops.object.export_scene=None
bpy.ops.export_scene.gltf(filepath=os.path.join(OUT,'c.glb'))
bpy.ops.import_scene.gltf(filepath=os.path.join(OUT,'c.glb'))
o=C.object; print("  imported collections", [c.name for c in o.users_collection], [c.name for c in C.scene.collection.children], 'active coll', C.collection.name)
bpy.ops.wm.read_factory_settings()
cube=D.objects['Cube']
p=bpy.ops.object.shape_key_add.get_rna_type().properties['from_mix']; print("  shape_key_add from_mix default", p.default)
bpy.ops.object.camera_add(); cam=C.object
bpy.ops.object.select_all(action='DESELECT'); cube.select_set(True); cam.select_set(True); C.view_layer.objects.active=cube
bpy.ops.object.mode_set(mode='EDIT'); print("  mixed-type edit objects_in_mode", [o.name for o in C.objects_in_mode], 'cam selected', cam.select_get()); bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT'); C.view_layer.objects.active=cube
tryop("join active not selected", lambda: bpy.ops.object.join())
cube.select_set(True); tryop("join only active", lambda: bpy.ops.object.join())
bpy.ops.mesh.primitive_cube_add(location=(3,0,0)); c2=C.object; c2.data=cube.data
bpy.ops.object.select_all(action='DESELECT'); cube.select_set(True); C.view_layer.objects.active=cube; cube.scale=(2,2,2)
tryop("transform_apply multi-user", lambda: bpy.ops.object.transform_apply(scale=True))
tryop("transform_apply isolate", lambda: (bpy.ops.object.transform_apply(scale=True, isolate_users=True), cube.data.name, c2.data.name))
print("  scene props", C.scene.render.image_settings.file_format, C.scene.render.use_file_extension, C.scene.render.film_transparent, C.scene.view_settings.view_transform, C.scene.display_settings.display_device, C.scene.render.fps_base, C.scene.frame_step)
print("  render engines", [i.identifier for i in C.scene.render.bl_rna.properties['engine'].enum_items])
print("  context.mode items", [i.identifier for i in bpy.types.Context.bl_rna.properties['mode'].enum_items])
print("  object.mode items", [i.identifier for i in bpy.types.Object.bl_rna.properties['mode'].enum_items])
print("  pivot items", [i.identifier for i in bpy.types.ToolSettings.bl_rna.properties['transform_pivot_point'].enum_items])
print("  vgroup weight", C.tool_settings.vertex_group_weight, 'double_threshold', C.tool_settings.double_threshold, 'automerge', C.tool_settings.use_mesh_automerge)
