# WP 2.13 workflow (table): a flat box as the top, its bottom face cut into a 3x3 grid, the
# four corner cells inset individually and extruded down as legs, normals recalculated.
# Side quads of the extrusion follow Blender's pointer-hash order (D-65): canonical order.
# golden: startup=empty compare=objects,context,mesh:Table tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1), scale=(1.2, 0.8, 0.05))
bpy.context.object.name = 'Table'
bpy.context.object.data.name = 'Table'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[4])
bpy.ops.mesh.subdivide(number_cuts=2)
bpy.data.meshes['Table'].total_face_sel
bpy.ops.frap.select_elements(type='FACE', indices=[4, 6, 8, 12])
bpy.ops.mesh.inset(thickness=0.08, use_individual=True)
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, -0.9)})
bpy.data.meshes['Table'].total_face_sel
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.normals_make_consistent(inside=False)
