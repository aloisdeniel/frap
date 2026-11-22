# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001,mesh:Sphere tier=T2 tol=0.0001
# fuzz: seed=0xb3c281483f54082f (replay)
import bpy
bpy.context.object.modifiers['Wireframe'].thickness = 0.16
bpy.context.object.modifiers['Array'].count = 3
bpy.ops.object.make_links_data(type='OBDATA')
bpy.ops.object.shape_key_clear()
bpy.ops.object.shade_smooth(keep_sharp_edges=True)
bpy.ops.object.shape_key_remove(all=False, apply_mix=False)
bpy.ops.object.select_more()
bpy.ops.object.vertex_group_normalize_all()
bpy.ops.mesh.primitive_uv_sphere_add()
bpy.ops.mesh.primitive_cube_add(rotation=(-0.58, 0.39, 0.65))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_loose(extend=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.view3d.snap_cursor_to_center()
