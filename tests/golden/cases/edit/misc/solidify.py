# WP 2.10: mesh.solidify (needs extrude_face_region of WP 2.5 and reverse_faces of WP 2.9):
# the whole cube gives an inner shell (V16 F12, the originals flipped), the shell faces are
# selected and the history cleared; then two faces of a grid with a negative thickness.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T2
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.solidify(thickness=0.2)
bpy.data.meshes['Cube'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3, size=2, location=(4, 0, 0), enter_editmode=True)
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1])
bpy.ops.mesh.solidify(thickness=-0.1)
bpy.data.meshes['Grid'].total_face_sel
