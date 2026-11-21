# M5+M6 stabilization round 5, object.duplicate (USER_DUP_ACT): a copied mesh's shape keys get
# a copy of their action, so keying the copy's shape key leaves the original's animation alone.
# K is keyed 0@1 on the Cube, the Cube duplicated, the copy's K keyed 1@10, frame_set(10): the
# Cube's K stays 0, the copy's is 1. The probe P_Keys holds (Cube K, Cube.001 K, index of the
# last action); P_Actions the index of each key's action in bpy.data.actions and their users.
# golden: startup=factory compare=objects tier=T1 tol=1e-6
import bpy
bpy.data.objects['Cube'].shape_key_add(name='Basis')
bpy.data.objects['Cube'].shape_key_add(name='K')
bpy.data.meshes['Cube'].shape_keys.key_blocks['K'].value = 0.0
bpy.data.meshes['Cube'].shape_keys.key_blocks['K'].keyframe_insert('value', frame=1)
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.duplicate()
bpy.data.meshes['Cube.001'].shape_keys.key_blocks['K'].value = 1.0
bpy.data.meshes['Cube.001'].shape_keys.key_blocks['K'].keyframe_insert('value', frame=10)
bpy.context.scene.frame_set(10)
bpy.data.objects.new('P_Keys', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Keys'])
bpy.data.objects['P_Keys'].location = (bpy.data.meshes['Cube'].shape_keys.key_blocks['K'].value, bpy.data.meshes['Cube.001'].shape_keys.key_blocks['K'].value, bpy.data.actions.find(bpy.data.actions[-1].name))
bpy.data.objects.new('P_Actions', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Actions'])
bpy.data.objects['P_Actions'].location = (bpy.data.actions.find(bpy.data.meshes['Cube'].shape_keys.animation_data.action.name), bpy.data.actions.find(bpy.data.meshes['Cube.001'].shape_keys.animation_data.action.name), 0.0)
bpy.data.objects['P_Actions'].scale = (bpy.data.meshes['Cube'].shape_keys.animation_data.action.users, bpy.data.meshes['Cube.001'].shape_keys.animation_data.action.users, 1.0)
