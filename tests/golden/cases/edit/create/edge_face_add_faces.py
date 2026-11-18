# WP 2.8 (needs 2.7 dissolve_faces): mesh.edge_face_add with faces selected (F on the default
# cube with everything selected, then on two adjacent faces).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.edge_face_add()
bpy.data.meshes['Cube'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.delete()
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=2, size=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.edge_face_add()
bpy.data.meshes['Grid'].total_face_sel
