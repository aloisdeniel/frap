# WP 8.5: object.parent_set(type='ARMATURE_AUTO') with a three-bone chain inside a subdivided
# box (moved, rotated, scaled; the armature moved too): bone heat weights per group. Each
# group's weights are copied into the X offsets of an object sharing the mesh (DISPLACE,
# mid_level 0, strength 1, along X: offset = weight), so the evaluated W.* meshes compare the
# weights vertex by vertex within 1e-4; the posed mesh itself compares the whole deform.
# golden: startup=empty tier=T1 tol=1e-4 compare=objects,evaluated:Cube,evaluated:W.Bone,evaluated:W.Mid,evaluated:W.Top
import bpy
bpy.ops.object.armature_add(location=(0.1, 0.05, -1.4))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].tail = (0, 0, 0.9)
bpy.data.armatures['Armature'].edit_bones.new('Mid')
bpy.data.armatures['Armature'].edit_bones['Mid'].head = (0, 0, 0.9)
bpy.data.armatures['Armature'].edit_bones['Mid'].tail = (0.05, 0, 1.9)
bpy.data.armatures['Armature'].edit_bones['Mid'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['Mid'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('Top')
bpy.data.armatures['Armature'].edit_bones['Top'].head = (0.05, 0, 1.9)
bpy.data.armatures['Armature'].edit_bones['Top'].tail = (0, 0, 2.7)
bpy.data.armatures['Armature'].edit_bones['Top'].parent = bpy.data.armatures['Armature'].edit_bones['Mid']
bpy.data.armatures['Armature'].edit_bones['Top'].use_connect = True
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(size=1.0, location=(0.1, 0, 0), rotation=(0, 0, 0.3))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=5)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].scale = (0.6, 0.5, 3.0)
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.parent_set(type='ARMATURE_AUTO')
bpy.data.objects['Cube'].vertex_groups.keys()
#@blender assert bpy.data.objects['Cube'].vertex_groups.keys() == ['Bone', 'Mid', 'Top']
bpy.data.objects['Cube'].vertex_groups.active_index
#@blender assert bpy.data.objects['Cube'].vertex_groups.active_index == 2
bpy.data.objects.new('W.Bone', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Bone'])
bpy.data.objects['W.Bone'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Bone'].modifiers['D'].vertex_group = 'Bone'
bpy.data.objects['W.Bone'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Bone'].modifiers['D'].direction = 'X'
bpy.data.objects.new('W.Mid', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Mid'])
bpy.data.objects['W.Mid'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Mid'].modifiers['D'].vertex_group = 'Mid'
bpy.data.objects['W.Mid'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Mid'].modifiers['D'].direction = 'X'
bpy.data.objects.new('W.Top', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Top'])
bpy.data.objects['W.Top'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Top'].modifiers['D'].vertex_group = 'Top'
bpy.data.objects['W.Top'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Top'].modifiers['D'].direction = 'X'
bpy.data.objects['Armature'].pose.bones['Mid'].rotation_quaternion = (0.95, 0.3, 0.0, 0.0)
bpy.data.objects['Armature'].pose.bones['Top'].rotation_quaternion = (0.9, 0.0, 0.0, 0.42)
