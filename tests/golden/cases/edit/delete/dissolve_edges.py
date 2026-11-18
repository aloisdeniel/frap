# WP 2.7: mesh.dissolve_edges joins the faces on both sides of each selected edge; the cube's
# e0 gives V6 E9 F5 (the two-edge vertices are dissolved, research App. B.6). Dissolving a
# boundary edge changes nothing: WARNING 'No edges dissolved'.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.dissolve_edges()
bpy.ops.frap.select_elements(type='EDGE', indices=[1, 3])
bpy.ops.mesh.dissolve_edges(use_verts=False)
