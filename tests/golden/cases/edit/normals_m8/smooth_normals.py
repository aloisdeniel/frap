# WP 8.3.2: mesh.smooth_normals on Suzanne with sharp edges and custom normals from
# set_normals_from_faces: the default factor on a vertex selection, factor 1 on a face
# selection, factor 0 (unchanged up to re-encoding) and a mesh without custom normals (the
# layer is created).
# golden: startup=factory compare=objects,context,mesh:Suzanne,mesh:Suzanne.001 tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.shade_smooth()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.set_sharpness_by_angle(angle=0.9)
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[10, 11, 12, 13, 50, 51, 52, 200, 201, 202, 203, 300])
bpy.ops.mesh.set_normals_from_faces()
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.frap.select_elements(type='VERT', indices=[5, 6, 7, 8, 9, 10, 100, 101, 102, 250])
bpy.ops.mesh.smooth_normals()
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[11, 12, 201, 202, 300, 301])
bpy.ops.mesh.smooth_normals(factor=1.0)
bpy.ops.frap.select_elements(type='FACE', indices=[50, 51])
bpy.ops.mesh.smooth_normals(factor=0.0)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_monkey_add(location=(3, 0, 0))
bpy.ops.object.shade_smooth()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.smooth_normals(factor=0.75)
