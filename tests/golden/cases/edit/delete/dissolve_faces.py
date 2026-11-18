# WP 2.7: mesh.dissolve_faces merges each region of selected faces into one selected n-gon:
# a single face stays (V8 E12 F6), two adjacent faces give a hexagon (V8 E11 F5, research
# App. B.6); with use_verts the vertices left between two edges dissolve too.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[2])
bpy.ops.mesh.dissolve_faces()
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1])
bpy.ops.mesh.dissolve_faces()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 4, 5, 10, 11, 14])
bpy.ops.mesh.dissolve_faces(use_verts=True)
