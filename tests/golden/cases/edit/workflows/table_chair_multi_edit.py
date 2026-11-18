# WP 2.13 workflow (table and chair together): two slabs edited at once (multi-object Edit
# Mode): mesh.select_mode and frap.select_by_normal act on both meshes, so one inset and one
# extrusion give both a pedestal; then the chair alone (frap.select_elements with `object`)
# gets a seat recess.
# golden: startup=empty compare=objects,context,mesh:Table,mesh:Chair tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 0, 0.75), scale=(1.4, 0.8, 0.06))
bpy.context.object.name = 'Table'
bpy.context.object.data.name = 'Table'
bpy.ops.mesh.primitive_cube_add(size=1, location=(0, -0.9, 0.45), scale=(0.45, 0.45, 0.06))
bpy.context.object.name = 'Chair'
bpy.context.object.data.name = 'Chair'
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.objects_in_mode
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_by_normal(direction=(0, 0, -1))
bpy.data.meshes['Table'].total_face_sel
bpy.data.meshes['Chair'].total_face_sel
bpy.ops.mesh.inset(thickness=0.15)
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, -0.4)})
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[5], object='Chair')
bpy.data.meshes['Table'].total_face_sel
bpy.ops.mesh.inset(thickness=0.05, depth=-0.02)
