# WP 2.10: mesh.poke fans the selected faces around a new centre vertex: the cube's top face
# gives centre vertex 8 and triangles [7,3,8] [3,1,8] [1,5,8] [5,7,8] (the first in face
# index 5); then two faces with an offset along the normal, relative offset and BOUNDS and
# MEDIAN centres. The centres and fans are selected, the history cleared.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.poke()
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1])
bpy.ops.mesh.poke(offset=0.5, center_mode='BOUNDS')
bpy.ops.frap.select_elements(type='FACE', indices=[2, 3])
bpy.ops.mesh.poke(offset=-0.25, use_relative_offset=True, center_mode='MEDIAN')
bpy.data.meshes['Cube'].total_face_sel
