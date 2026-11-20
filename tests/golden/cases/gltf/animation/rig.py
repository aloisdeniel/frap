# M5+M6 W3 integration (WPs 6.7 + 6.8): an armature with keyed pose bones, a skinned mesh with
# an animated shape key, and an object parented to a bone. The export writes the bone channels
# on the joint nodes, the weights channel on the skinned mesh's node and the bone-parented
# object's node; the re-import puts the bone channels on `pose.bones["…"]` of the new armature
# and the weights on the new shape keys (actions `Wave` and `Grow`), and
# a second export writes the same animation.
# golden: startup=factory tier=T2 tol=1e-4 compare=glb:out/rig.glb,glb:out/again.glb,anim:Wave,anim:Grow,objects
import bpy
bpy.data.objects['Cube'].hide_render = True
bpy.ops.object.armature_add(location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones.new('Top')
bpy.data.armatures['Armature'].edit_bones['Top'].head = (0, 0, 1.0)
bpy.data.armatures['Armature'].edit_bones['Top'].tail = (0, 0, 2.0)
bpy.data.armatures['Armature'].edit_bones['Top'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Armature'].pose.bones['Top'].keyframe_insert('rotation_quaternion', frame=1)
bpy.data.objects['Armature'].pose.bones['Top'].rotation_quaternion = (0.9, 0.0, 0.4, 0.0)
bpy.data.objects['Armature'].pose.bones['Top'].keyframe_insert('rotation_quaternion', frame=10)
bpy.data.objects['Armature'].pose.bones['Bone'].keyframe_insert('location', frame=1)
bpy.data.objects['Armature'].pose.bones['Bone'].location = (0.0, 0.5, 0.0)
bpy.data.objects['Armature'].pose.bones['Bone'].keyframe_insert('location', frame=10)
bpy.data.objects['Armature'].animation_data.action.name = 'Wave'
bpy.ops.mesh.primitive_cylinder_add(vertices=6, radius=0.4, depth=2.0, location=(0.0, 0.0, 1.0))
bpy.data.objects['Cylinder'].vertex_groups.new(name='Bone')
bpy.data.objects['Cylinder'].vertex_groups.new(name='Top')
bpy.data.objects['Cylinder'].vertex_groups['Bone'].add([0, 2, 4, 6, 8, 10], 1.0, 'REPLACE')
bpy.data.objects['Cylinder'].vertex_groups['Top'].add([1, 3, 5, 7, 9, 11], 1.0, 'REPLACE')
bpy.data.objects['Cylinder'].modifiers.new(name='Armature', type='ARMATURE')
bpy.data.objects['Cylinder'].modifiers['Armature'].object = bpy.data.objects['Armature']
bpy.data.objects['Cylinder'].parent = bpy.data.objects['Armature']
bpy.data.objects['Cylinder'].shape_key_add(name='Basis')
bpy.data.objects['Cylinder'].shape_key_add(name='Bulge')
bpy.data.objects['Cylinder'].data.shape_keys.key_blocks['Bulge'].data[1].co = (0.8, 0.0, 1.2)
bpy.data.objects['Cylinder'].data.shape_keys.key_blocks['Bulge'].keyframe_insert('value', frame=1)
bpy.data.objects['Cylinder'].data.shape_keys.key_blocks['Bulge'].value = 1.0
bpy.data.objects['Cylinder'].data.shape_keys.key_blocks['Bulge'].keyframe_insert('value', frame=10)
bpy.data.shape_keys['Key'].animation_data.action.name = 'Grow'
bpy.ops.mesh.primitive_cube_add(size=0.3, location=(0.0, 0.0, 2.0))
bpy.data.objects['Cube.001'].parent = bpy.data.objects['Armature']
bpy.data.objects['Cube.001'].parent_type = 'BONE'
bpy.data.objects['Cube.001'].parent_bone = 'Top'
bpy.ops.export_scene.gltf(filepath='out/rig.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/rig.glb')
bpy.ops.export_scene.gltf(filepath='out/again.glb')
