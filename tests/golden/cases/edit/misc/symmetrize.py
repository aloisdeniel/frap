# WP 2.10: mesh.symmetrize (needs bisect_plane of WP 2.8, reverse_faces of WP 2.9 and
# weld_verts of WP 2.7): the monkey made symmetric from +X onto -X (V507 F500); a cube moved
# off-centre in Edit Mode made symmetric from -Y onto +Y with a larger snap threshold; the
# result is selected.
# golden: startup=factory compare=objects,context,mesh:Suzanne,mesh:Cube tier=T2
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.primitive_cube_add(size=1, location=(0.3, 0.2, 0.1))
bpy.ops.mesh.symmetrize(direction='NEGATIVE_Y', threshold=0.05)
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_monkey_add(location=(4, 0, 0), enter_editmode=True)
bpy.ops.mesh.symmetrize(direction='POSITIVE_X')
bpy.data.meshes['Suzanne'].total_vert_sel
