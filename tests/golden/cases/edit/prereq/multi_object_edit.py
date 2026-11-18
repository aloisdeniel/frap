# WP 2.2: multi-object Edit Mode, ending in Edit Mode: the per-object selection after
# select_all and selection by index on the active and on another edit object.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0), size=1)
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='TOGGLE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 5])
bpy.ops.frap.select_elements(type='VERT', indices=[7], object='Cube')
bpy.ops.mesh.select_all(action='INVERT')
