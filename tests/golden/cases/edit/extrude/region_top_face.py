# WP 2.5: mesh.extrude_region on the cube's top face (face 0 of the factory cube): the face is
# replaced by its copy (the cap), four side quads join the copy to the old boundary, the cap is
# selected and the history follows it. Side-quad order is run-dependent in Blender (GHash):
# compared order-free.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 order=canonical
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.extrude_region()
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
