# M5+M6 stabilization round 5: RiggedSimple (CC-BY-4.0, Cesium; LICENSE.md beside it) imported,
# everything duplicated, exported (ACTIONS mode). object.duplicate copies the armature's
# action Anim_0 to Anim_0.001 and remaps the copy's NLA strip and the copied cylinder's
# Armature modifier to the copies, so the export writes two animations of 6 channels each
# (the duplicate-action fix: the copy's strip kept Anim_0, which then animated both rigs).
# P_Anim holds the users of Anim_0 and Anim_0.001 and the index of the copy's strip action,
# its scale the index of the copied cylinder's last modifier and of its Armature object.
# golden: startup=empty order=canonical tier=T2 tol=1e-5 compare=objects,glb:out/dup.glb glb_structure
import bpy
bpy.ops.import_scene.gltf(filepath='RiggedSimple.glb')
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.duplicate()
bpy.data.objects.new('P_Anim', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Anim'])
bpy.data.objects['P_Anim'].location = (bpy.data.actions['Anim_0'].users, bpy.data.actions['Anim_0.001'].users, bpy.data.actions.find(bpy.data.objects['Armature.001'].animation_data.nla_tracks[0].strips[0].action.name))
bpy.data.objects['P_Anim'].scale = (bpy.data.objects['Cylinder.001'].modifiers.find(bpy.data.objects['Cylinder.001'].modifiers[-1].name), bpy.data.objects.find(bpy.data.objects['Cylinder.001'].modifiers[0].object.name), 1.0)
bpy.ops.export_scene.gltf(filepath='out/dup.glb')
