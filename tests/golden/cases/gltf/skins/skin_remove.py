# WP 6.8: export_armature_object_remove on a single-root rig (fully weighted mesh, an object
# parented to a bone): the armature object is dropped, its root bone becomes a scene root with
# the world transform, the skinned mesh a root too. A second armature with no skinned object
# gives an unused skin. Compared with the default export.
# golden: startup=factory tier=T2 tol=1e-5 compare=glb:out/kept.glb,glb:out/removed.glb glb_structure
import bpy
bpy.ops.object.armature_add(location=(0.5, 0, -1), rotation=(0, 0.2, 0.5))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].tail = (0, 0, 1.0)
bpy.data.armatures['Armature'].edit_bones.new('Mid')
bpy.data.armatures['Armature'].edit_bones['Mid'].head = (0, 0, 1.0)
bpy.data.armatures['Armature'].edit_bones['Mid'].tail = (0, 0.2, 2.0)
bpy.data.armatures['Armature'].edit_bones['Mid'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['Mid'].use_connect = True
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cylinder_add(vertices=6, radius=0.5, depth=2.0, location=(0.5, 0, 0.0))
bpy.data.objects['Cylinder'].vertex_groups.new(name='Bone')
bpy.data.objects['Cylinder'].vertex_groups.new(name='Mid')
bpy.data.objects['Cylinder'].vertex_groups['Bone'].add([0, 2, 4, 6, 8, 10], 1.0, 'REPLACE')
bpy.data.objects['Cylinder'].vertex_groups['Mid'].add([1, 3, 5, 7, 9, 11], 0.7, 'REPLACE')
bpy.data.objects['Cylinder'].vertex_groups['Bone'].add([1, 3], 0.2, 'REPLACE')
bpy.data.objects['Cylinder'].modifiers.new(name='Armature', type='ARMATURE')
bpy.data.objects['Cylinder'].modifiers['Armature'].object = bpy.data.objects['Armature']
bpy.data.objects['Cylinder'].parent = bpy.data.objects['Armature']
bpy.data.objects['Cube'].scale = (0.3, 0.3, 0.3)
bpy.data.objects['Cube'].parent = bpy.data.objects['Armature']
bpy.data.objects['Cube'].parent_type = 'BONE'
bpy.data.objects['Cube'].parent_bone = 'Mid'
bpy.data.objects['Armature'].pose.bones['Mid'].rotation_quaternion = (0.9, 0.3, 0.0, 0.1)
bpy.ops.object.armature_add(location=(3, 0, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.export_scene.gltf(filepath='out/kept.glb')
bpy.ops.export_scene.gltf(filepath='out/removed.glb', export_armature_object_remove=True)
