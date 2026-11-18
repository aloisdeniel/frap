# WP 2.7: mesh.merge(type='CENTER') merges the selected vertices at their mean into the
# lowest-index one (research App. B.6: v0, v1, v3 -> V6 E9 F5); 'CURSOR' at the 3D cursor in
# object space; the survivor stays selected.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 3])
bpy.ops.mesh.merge(type='CENTER')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0), rotation=(0.3, 0, 0.5), scale=(1, 2, 1))
bpy.context.scene.cursor.location = (4, 1, 2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[2, 4, 6])
bpy.ops.mesh.merge(type='CURSOR')
bpy.ops.mesh.merge()
