# WP 2.13 workflow (chair): a seat slab whose top and bottom faces are both cut into 3x3
# grids in one subdivide; the four bottom corner cells are extruded down as legs, then the
# back row of top cells (untouched by the first extrusion, so their indices hold) is extruded
# up as the backrest and pushed back a little.
# golden: startup=empty compare=objects,context,mesh:Chair tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 0, 0.45), scale=(1, 1, 0.1))
bpy.context.object.name = 'Chair'
bpy.context.object.data.name = 'Chair'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[4, 5])
bpy.ops.mesh.subdivide(number_cuts=2)
bpy.ops.frap.select_elements(type='FACE', indices=[4, 14, 16, 20])
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, -0.4)})
bpy.ops.frap.select_elements(type='FACE', indices=[8, 10, 12])
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, 0.5)})
bpy.ops.transform.translate(value=(0, 0.05, 0))
bpy.data.meshes['Chair'].total_face_sel
