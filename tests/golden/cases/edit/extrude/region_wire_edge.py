# WP 2.5: mesh.extrude_region in vertex mode with one selected edge (two vertices): the edge is
# extruded as a region (one side quad, wound like the adjacent faces).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[7])
bpy.ops.mesh.extrude_region(use_normal_flip=True)
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
