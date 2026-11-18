# WP 2.10: mesh.separate(type='SELECTED') moves the selected top face into a new object
# Cube.001 with a new mesh Cube.001 (selected, not in Edit Mode); the edit object stays
# active and in Edit Mode, keeps the vertices still used by its other faces and is
# deselected. A second separation (an edge and a vertex) gives Cube.002.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001,mesh:Cube.002 tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.separate()
bpy.context.mode
bpy.data.objects['Cube.001'].mode
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.frap.select_elements(type='VERT', indices=[7], action='ADD')
bpy.ops.mesh.separate(type='SELECTED')
bpy.data.objects['Cube.002'].select_get()
bpy.data.objects['Cube'].select_get()
