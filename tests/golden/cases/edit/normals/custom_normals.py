# WP 2.9: customdata_custom_splitnormals_add/clear in Object and Edit Mode (cancelled when
# there is nothing to do), set_normals_from_faces (with and without keep_sharp), and
# flip_normals carrying the custom normals (face winding and only_clnors).
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.customdata_custom_splitnormals_add()
bpy.ops.mesh.customdata_custom_splitnormals_add()
bpy.data.meshes['Cube'].has_custom_normals
bpy.ops.mesh.customdata_custom_splitnormals_clear()
bpy.ops.mesh.customdata_custom_splitnormals_clear()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.customdata_custom_splitnormals_add()
bpy.ops.mesh.customdata_custom_splitnormals_clear()
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[5, 2])
bpy.ops.mesh.set_normals_from_faces()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(3, 0, 0))
bpy.ops.object.shade_smooth()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 7])
bpy.ops.mesh.set_normals_from_faces(keep_sharp=True)
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.mesh.flip_normals()
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.frap.select_elements(type='VERT', indices=[2, 3])
bpy.ops.mesh.flip_normals(only_clnors=True)
