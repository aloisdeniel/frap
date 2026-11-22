# WP 8.2: two of the three edges at cube corners beveled with three segments (an
# unbeveled edge in the vertex mesh: boundary vertices on that edge).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 1, 3])
bpy.ops.mesh.bevel(offset=0.3, segments=3)
