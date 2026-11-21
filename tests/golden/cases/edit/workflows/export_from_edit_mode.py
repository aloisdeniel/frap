# M2 stabilization: exporting while a mesh is in Edit Mode. The glTF add-on calls
# `object.mode_set(mode='OBJECT')` first (`export.save`), so the edits are written back, the GLB
# holds the extruded cube and the session ends in Object Mode (compared through `context`).
# The second object in Edit Mode (multi-object edit) is written back too.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cyl,glb:out/edit.glb tier=T1 order=canonical glb_structure
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8, radius=0.5, depth=1, location=(3, 0, 0))
bpy.context.object.name = 'Cyl'
bpy.context.object.data.name = 'Cyl'
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_by_normal(direction=(0, 0, 1), threshold=0.1)
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, 1)})
bpy.ops.export_scene.gltf(filepath='out/edit.glb')
