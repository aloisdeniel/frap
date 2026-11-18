# WP 2.2: calc_uvs in Edit Mode: on a mesh without UV map the operator adds `UVMap` (the faces
# selected at that moment get default UVs), then writes the primitive's UVs to the active UV
# map; calc_uvs=False leaves the new corners at (0, 0).
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=2, y_subdivisions=2, calc_uvs=False)
bpy.ops.mesh.primitive_plane_add(location=(0, 0, 1), calc_uvs=False)
bpy.ops.object.delete()
bpy.context.view_layer.objects.active = bpy.data.objects['Grid']
bpy.data.objects['Grid'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[1, 2])
bpy.ops.mesh.primitive_cube_add(size=0.5, location=(0, 0, 1))
bpy.ops.mesh.primitive_plane_add(size=0.5, location=(2, 0, 0), calc_uvs=False)
bpy.ops.mesh.primitive_circle_add(vertices=5, fill_type='NGON', location=(-2, 0, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].uv_layers.active.name
