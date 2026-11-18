# WP 2.13 workflow (stool): a square seat whose bottom is cut into a 3x3 grid; the four
# corner cells are extruded down, then their end faces (still selected) are collapsed each to
# a point with mesh.merge(type='COLLAPSE'), giving pointed legs.
# golden: startup=empty compare=objects,context,mesh:Cube tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 0, 0.5), scale=(1, 1, 0.1))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[4])
bpy.ops.mesh.subdivide(number_cuts=2)
bpy.ops.frap.select_elements(type='FACE', indices=[4, 6, 8, 12])
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, -0.45)})
bpy.ops.mesh.merge(type='COLLAPSE')
bpy.data.meshes['Cube'].total_vert_sel
