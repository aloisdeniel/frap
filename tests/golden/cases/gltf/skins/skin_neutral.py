# WP 6.8: a skinned mesh that is not the armature's child (its armature found by the
# modifier's object name), with unweighted vertices bound to the 'neutral_bone' joint added
# under the armature node; a second mesh parented to the armature with an ARMATURE modifier
# but no vertex groups (no skin on its node, its own glTF mesh). (With
# export_armature_object_remove the add-on fails here: `add_neutral_bones` appends to the removed
# armature's node, None; see skin_remove.py.)
# golden: startup=factory tier=T2 tol=1e-5 compare=glb:out/neutral.glb glb_structure
import bpy
bpy.ops.object.armature_add(location=(0, 0, -1), rotation=(0, 0, 0.5))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].tail = (0, 0, 1.0)
bpy.data.armatures['Armature'].edit_bones.new('Mid')
bpy.data.armatures['Armature'].edit_bones['Mid'].head = (0, 0, 1.0)
bpy.data.armatures['Armature'].edit_bones['Mid'].tail = (0, 0.2, 2.0)
bpy.data.armatures['Armature'].edit_bones['Mid'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['Mid'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('End')
bpy.data.armatures['Armature'].edit_bones['End'].head = (0, 0.2, 2.0)
bpy.data.armatures['Armature'].edit_bones['End'].tail = (0, 0.2, 3.0)
bpy.data.armatures['Armature'].edit_bones['End'].parent = bpy.data.armatures['Armature'].edit_bones['Mid']
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cylinder_add(vertices=6, radius=0.5, depth=3.0, location=(0.5, 0, 0.5))
bpy.data.objects['Cylinder'].vertex_groups.new(name='Bone')
bpy.data.objects['Cylinder'].vertex_groups.new(name='End')
bpy.data.objects['Cylinder'].vertex_groups['Bone'].add([0, 2, 4, 6], 1.0, 'REPLACE')
bpy.data.objects['Cylinder'].vertex_groups['End'].add([1, 3, 5, 7], 0.7, 'REPLACE')
bpy.data.objects['Cylinder'].modifiers.new(name='Armature', type='ARMATURE')
bpy.data.objects['Cylinder'].modifiers['Armature'].object = bpy.data.objects['Armature']
bpy.data.objects['Cube'].scale = (0.3, 0.3, 0.3)
bpy.data.objects['Cube'].location = (-1.0, 0, 0)
bpy.data.objects['Cube'].modifiers.new(name='Armature', type='ARMATURE')
bpy.data.objects['Cube'].modifiers['Armature'].object = bpy.data.objects['Armature']
bpy.data.objects['Cube'].parent = bpy.data.objects['Armature']
bpy.data.objects['Armature'].pose.bones['Mid'].rotation_quaternion = (0.9, 0.3, 0.0, 0.1)
bpy.ops.export_scene.gltf(filepath='out/neutral.glb')
