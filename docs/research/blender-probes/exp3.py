import bpy
C=bpy.context; D=bpy.data
def r(v): return tuple(round(x,4) for x in v)
cube=D.objects['Cube']; cube.location=(2,0,0)
bpy.ops.mesh.primitive_cone_add(location=(0,3,0)); cone=C.object
cube.select_set(True); C.view_layer.objects.active=cube; bpy.ops.object.parent_set()
bpy.ops.object.select_all(action='DESELECT'); cube.select_set(True); bpy.ops.object.delete()
print("child world after parent delete", r(cone.matrix_world.translation), r(cone.location), cone.parent)
bpy.ops.image.new(); im=D.images['Untitled']; print("image.new", im.name, tuple(im.size), im.generated_type, r(im.generated_color), im.use_generated_float, im.alpha_mode, im.colorspace_settings.name, im.source)
print("startup mats", [m.name for m in D.materials], "worlds", [w.name for w in D.worlds], [ (n.name, n.type) for n in D.worlds[0].node_tree.nodes], "collections", [c.name for c in D.collections], [o.name for o in C.scene.collection.objects])
bpy.ops.mesh.primitive_cube_add(); n1=C.object.name
bpy.ops.mesh.primitive_cube_add(); n2=C.object.name
D.objects[n1].name='Zzz'; bpy.ops.mesh.primitive_cube_add(); print("names", n1, n2, C.object.name)
print("new obj collection", [c.name for c in C.object.users_collection], "active collection", C.collection.name)
print("light startup", D.lights['Light'].type, D.lights['Light'].energy)
o=C.object; print("rotation_mode", o.rotation_mode, "quat", r(o.rotation_quaternion))
print("ts", C.tool_settings.transform_pivot_point, C.tool_settings.use_proportional_edit, C.tool_settings.mesh_select_mode[:], C.tool_settings.use_snap, C.tool_settings.use_keyframe_insert_auto)
print("cursor props", [p.identifier for p in C.scene.cursor.bl_rna.properties])
