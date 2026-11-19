# WP 4.7: linked-duplicate mesh sharing with export_apply=True (the add-on's `__gather_mesh`
# and `get_data_cache_key`). Four objects use the mesh 'Cube': 'Cube' (no modifiers) and 'F'
# (no modifiers, an OBJECT-linked slot) share the accessors of one data mesh, with their own
# materials; 'Cube.001' (Subdivision) and 'Cube.002' (a Bevel disabled in the viewport) each get
# a glTF mesh of their own named 'Cube', with the evaluated mesh's materials (the OBJECT-linked
# slot of 'Cube.002' is not applied) and fresh vertex accessors; index accessors are shared by
# content.
# golden: startup=factory tier=T2 compare=glb:out/sharing.glb tol=1e-4
import bpy
bpy.ops.object.duplicate_move_linked(TRANSFORM_OT_translate={'value': (3, 0, 0)})
bpy.ops.object.duplicate_move_linked(TRANSFORM_OT_translate={'value': (3, 0, 0)})
bpy.data.objects.new('F', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['F'])
bpy.data.objects['F'].location = (0, 3, 0)
bpy.data.objects['Cube.001'].modifiers.new(name='Sub', type='SUBSURF')
bpy.data.objects['Cube.002'].modifiers.new(name='Bev', type='BEVEL')
bpy.data.objects['Cube.002'].modifiers['Bev'].show_viewport = False
bpy.data.materials.new(name='Red')
bpy.data.materials.new(name='Blue')
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['Red'])
bpy.data.objects['F'].material_slots[0].link = 'OBJECT'
bpy.data.objects['F'].material_slots[0].material = bpy.data.materials['Blue']
bpy.data.objects['Cube.002'].material_slots[0].link = 'OBJECT'
bpy.data.objects['Cube.002'].material_slots[0].material = bpy.data.materials['Blue']
bpy.ops.export_scene.gltf(filepath='out/sharing.glb', export_apply=True)
