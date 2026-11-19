# WP 4.7: export_apply=True with objects outside the viewport depsgraph. An object disabled in
# viewports (`Object.hide_viewport`, or every collection linking it `hide_viewport`) is not
# evaluated: `evaluated_get(depsgraph).to_mesh()` is its data, so 'Cube' (Subdivision) exports
# the 24-vertex cube, still as a glTF mesh of its own (fresh vertex accessors, index accessor
# shared by content with its linked duplicate 'Cube.001') with the data's material (its
# OBJECT-linked 'Blue' slot is not applied). A hidden Boolean operand ('Op', Subdivision) is
# pulled into the depsgraph by the visible 'Cube.001' and is evaluated.
# golden: startup=factory tier=T2 compare=glb:out/object.glb,glb:out/collection.glb,glb:out/operand.glb tol=1e-4
import bpy
bpy.ops.object.duplicate_move_linked(TRANSFORM_OT_translate={'value': (3, 0, 0)})
bpy.data.objects['Cube'].modifiers.new(name='Sub', type='SUBSURF')
bpy.data.materials.new(name='Blue')
bpy.data.objects['Cube'].material_slots[0].link = 'OBJECT'
bpy.data.objects['Cube'].material_slots[0].material = bpy.data.materials['Blue']
bpy.data.objects['Cube'].hide_viewport = True
bpy.ops.export_scene.gltf(filepath='out/object.glb', export_apply=True)
bpy.data.objects['Cube'].hide_viewport = False
bpy.data.collections['Collection'].hide_viewport = True
bpy.ops.export_scene.gltf(filepath='out/collection.glb', export_apply=True)
bpy.data.collections['Collection'].hide_viewport = False
bpy.ops.mesh.primitive_cube_add(size=1, location=(3.5, 0.5, 0.5))
bpy.context.active_object.name = 'Op'
bpy.data.objects['Op'].modifiers.new(name='Sub', type='SUBSURF')
bpy.data.objects['Op'].hide_viewport = True
bpy.data.objects['Cube.001'].modifiers.new(name='Bool', type='BOOLEAN')
bpy.data.objects['Cube.001'].modifiers['Bool'].object = bpy.data.objects['Op']
bpy.data.objects['Cube.001'].modifiers['Bool'].solver = 'MANIFOLD'
bpy.ops.export_scene.gltf(filepath='out/operand.glb', export_apply=True)
