# WP 2.7: mesh.merge(type='FIRST'/'LAST') merges into the first/last vertex of the selection
# history, which keeps its slot and position; the items are dynamic: without a vertex at
# that end of the history the value is refused with Blender's conversion TypeError.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.merge(type='FIRST')
bpy.ops.mesh.merge(type='LAST')
bpy.ops.frap.select_elements(type='VERT', indices=[3, 0, 1])
bpy.ops.mesh.merge(type='FIRST')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[4, 0, 6])
bpy.ops.mesh.merge(type='LAST')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.merge(type='LAST')
