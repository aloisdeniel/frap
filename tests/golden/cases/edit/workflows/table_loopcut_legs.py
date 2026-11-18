# WP 2.13 workflow (table): a block cut by two loop cuts across X and two across Y, so the
# bottom is a 3x3 grid of quads; the four corner quads are extruded down as legs with the
# extrude-faces macro (each face on its own, shrink/fatten along its normal), then the leg
# ends are scaled about their own centres (pivot INDIVIDUAL_ORIGINS) to taper the legs.
# golden: startup=empty compare=objects,context,mesh:Cube tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1), scale=(1, 0.6, 0.05))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.loopcut(number_cuts=2, edge_index=10, object_index=0)
bpy.ops.mesh.loopcut(number_cuts=2, edge_index=0, object_index=0)
bpy.data.meshes['Cube'].total_edge_sel
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[4, 8, 16, 24])
bpy.ops.mesh.extrude_faces_move(TRANSFORM_OT_shrink_fatten={"value": 0.95})
bpy.data.meshes['Cube'].total_face_sel
bpy.context.scene.tool_settings.transform_pivot_point = 'INDIVIDUAL_ORIGINS'
bpy.ops.transform.resize(value=(0.6, 0.6, 1))
