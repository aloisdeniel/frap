# WP 7.7: inverse-bind-matrix accessor sharing on a bigger rig set (deviations §2: a review
# reported two skins with equal matrices written as two accessors). The add-on's @cached
# gather_accessor keys on the matrix bytes, so skins share one accessor exactly when their
# matrices are bit-identical. Rigs: a three-bone armature with a skinned cube; a duplicate (same
# place: shared); a linked duplicate (shared armature data) moved to x=3 (its own); a duplicate
# of that one moved back to the origin (shared with the first); one parented to an empty at the
# origin (identity parent: shared); two rigs rotated by the same 90 degrees (shared with each
# other); one whose cube has a vertex outside every group (the neutral bone: its own accessor,
# the matrices without the neutral one still shared); RiggedSimple imported twice (shared).
# Exports: default, Z-up, posed bones, deform bones only. Blender 5.2.4 writes 64 accessors,
# ten skins over five IBM accessors in each; frap the same (ruled out, deviations §2).
# golden: startup=factory order=canonical tier=T2 tol=1e-5 compare=objects,glb:out/rigs.glb,glb:out/zup.glb,glb:out/pose.glb,glb:out/deform.glb glb_structure
import bpy
bpy.data.objects.remove(bpy.data.objects['Camera'])
bpy.data.objects.remove(bpy.data.objects['Light'])
bpy.ops.object.armature_add(location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].tail = (0, 0.1, 1.0)
bpy.data.armatures['Armature'].edit_bones.new('Upper')
bpy.data.armatures['Armature'].edit_bones['Upper'].head = (0, 0.1, 1.0)
bpy.data.armatures['Armature'].edit_bones['Upper'].tail = (0.1, 0.0, 2.0)
bpy.data.armatures['Armature'].edit_bones['Upper'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['Upper'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('Side')
bpy.data.armatures['Armature'].edit_bones['Side'].head = (0, 0.1, 1.0)
bpy.data.armatures['Armature'].edit_bones['Side'].tail = (1.0, 0.1, 1.2)
bpy.data.armatures['Armature'].edit_bones['Side'].roll = -0.5
bpy.data.armatures['Armature'].edit_bones['Side'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['Side'].use_deform = False
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].vertex_groups.new(name='Bone')
bpy.data.objects['Cube'].vertex_groups.new(name='Upper')
bpy.data.objects['Cube'].vertex_groups['Bone'].add([0, 1, 2, 3], 1.0, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['Upper'].add([4, 5, 6, 7], 1.0, 'REPLACE')
bpy.data.objects['Cube'].modifiers.new(name='Armature', type='ARMATURE')
bpy.data.objects['Cube'].modifiers['Armature'].object = bpy.data.objects['Armature']
bpy.data.objects['Cube'].parent = bpy.data.objects['Armature']
bpy.data.objects['Armature'].pose.bones['Upper'].rotation_quaternion = (0.95, 0.1, 0.2, 0.0)
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.duplicate()
bpy.ops.object.duplicate(linked=True)
bpy.data.objects['Armature.002'].location = (3.0, 0.0, 0.0)
bpy.ops.object.duplicate()
bpy.data.objects['Armature.003'].location = (0.0, 0.0, 0.0)
bpy.ops.object.duplicate()
bpy.data.objects.new('Holder', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Holder'])
bpy.data.objects['Armature.004'].parent = bpy.data.objects['Holder']
bpy.ops.object.duplicate()
bpy.data.objects['Armature.005'].parent = None
bpy.data.objects['Armature.005'].rotation_euler = (0.0, 0.0, 1.5707964)
bpy.ops.object.duplicate()
bpy.data.objects['Armature.006'].location = (0.0, -3.0, 0.0)
bpy.data.objects['Armature.006'].location = (0.0, 0.0, 0.0)
bpy.ops.object.duplicate()
bpy.data.objects['Armature.007'].rotation_euler = (0.0, 0.0, 0.0)
bpy.data.objects['Cube.007'].vertex_groups['Bone'].remove([0])
bpy.ops.import_scene.gltf(filepath='RiggedSimple.glb')
bpy.ops.import_scene.gltf(filepath='RiggedSimple.glb')
bpy.ops.export_scene.gltf(filepath='out/rigs.glb')
bpy.ops.export_scene.gltf(filepath='out/zup.glb', export_yup=False)
bpy.ops.export_scene.gltf(filepath='out/pose.glb', export_rest_position_armature=False)
bpy.ops.export_scene.gltf(filepath='out/deform.glb', export_def_bones=True)
