# WP 2.9: the edit-mode normal and mark operators act on every mesh in Edit Mode (unique
# data): two cubes edited together, with different selections; attribute_set converts the
# value to each mesh's attribute type (FLOAT on the active mesh, INT on the other).
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(size=1, location=(3, 0, 0))
bpy.ops.geometry.attribute_add(name='w', domain='FACE', data_type='INT')
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.geometry.attribute_add(name='w', domain='FACE', data_type='FLOAT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[1, 2])
bpy.ops.frap.select_elements(type='FACE', indices=[4], object='Cube', action='SET')
bpy.ops.mesh.flip_normals()
bpy.ops.mesh.mark_seam()
bpy.ops.mesh.mark_sharp()
bpy.ops.mesh.faces_shade_smooth()
bpy.ops.mesh.attribute_set(value_float=2.75)
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.normals_make_consistent()
bpy.ops.mesh.set_sharpness_by_angle(angle=1.0)
