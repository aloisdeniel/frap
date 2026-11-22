# WP 8.5: object.parent_set(type='ARMATURE_AUTO') with one deforming bone (and a
# non-deforming one, which gets no group) on two selected meshes at once: a UV sphere the bone
# runs through and out of, and a cylinder beside it with an existing group named after the
# bone (reused) and one unrelated group (kept). One heat source weights every vertex fully.
# Weights are copied into X offsets (DISPLACE along X, mid_level 0: offset = weight). The
# canonical order: frap's UV sphere lists its faces in another order (not this WP's).
# golden: startup=empty tier=T1 tol=1e-4 order=canonical compare=objects,evaluated:W.Sphere,evaluated:W.Cylinder,evaluated:Sphere,evaluated:Cylinder
import bpy
bpy.ops.object.armature_add(location=(0, 0, -0.5))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].tail = (0.2, 0, 1.8)
bpy.data.armatures['Armature'].edit_bones.new('Helper')
bpy.data.armatures['Armature'].edit_bones['Helper'].head = (1, 0, 0)
bpy.data.armatures['Armature'].edit_bones['Helper'].tail = (1, 0, 1)
bpy.data.armatures['Armature'].edit_bones['Helper'].use_deform = False
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_uv_sphere_add(segments=16, ring_count=8, radius=0.8, location=(0, 0, 0.2))
bpy.ops.mesh.primitive_cylinder_add(vertices=10, radius=0.3, depth=1.5, location=(1.6, 0.2, 0.3), rotation=(0.2, 0, 0))
bpy.data.objects['Cylinder'].vertex_groups.new(name='Other')
bpy.data.objects['Cylinder'].vertex_groups['Other'].add([0, 1, 2], 0.3, 'REPLACE')
bpy.data.objects['Cylinder'].vertex_groups.new(name='Bone')
bpy.data.objects['Cylinder'].vertex_groups['Bone'].add([3, 4], 0.1, 'REPLACE')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Sphere'].select_set(True)
bpy.data.objects['Cylinder'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.parent_set(type='ARMATURE_AUTO')
bpy.data.objects['Sphere'].vertex_groups.keys()
#@blender assert bpy.data.objects['Sphere'].vertex_groups.keys() == ['Bone']
bpy.data.objects['Cylinder'].vertex_groups.keys()
#@blender assert bpy.data.objects['Cylinder'].vertex_groups.keys() == ['Other', 'Bone']
bpy.data.objects['Cylinder'].vertex_groups.active_index
#@blender assert bpy.data.objects['Cylinder'].vertex_groups.active_index == 1
bpy.data.objects.new('W.Sphere', bpy.data.meshes['Sphere'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Sphere'])
bpy.data.objects['W.Sphere'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Sphere'].modifiers['D'].vertex_group = 'Bone'
bpy.data.objects['W.Sphere'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Sphere'].modifiers['D'].direction = 'X'
bpy.data.objects.new('W.Cylinder', bpy.data.meshes['Cylinder'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Cylinder'])
bpy.data.objects['W.Cylinder'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Cylinder'].modifiers['D'].vertex_group = 'Bone'
bpy.data.objects['W.Cylinder'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Cylinder'].modifiers['D'].direction = 'X'
bpy.data.objects['W.Cylinder'].modifiers.new(name='O', type='DISPLACE')
bpy.data.objects['W.Cylinder'].modifiers['O'].vertex_group = 'Other'
bpy.data.objects['W.Cylinder'].modifiers['O'].mid_level = 0.0
bpy.data.objects['W.Cylinder'].modifiers['O'].direction = 'Y'
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.9, 0.2, 0.3, 0.0)
bpy.data.objects['Armature'].pose.bones['Bone'].location = (0.1, 0.0, 0.2)
