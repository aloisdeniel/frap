# WP 6.8: a skinned mesh with shape keys (targets in world space like the mesh), on a scaled,
# rotated object, and a second skinned mesh with a Subdivision modifier. Exports: default (the
# modifier ignored); export_apply (the ARMATURE modifier is switched off while evaluating, the
# Subdivision applied and its mesh's shape keys lost, the first mesh keeps its keys).
# golden: startup=factory tier=T2 tol=1e-5 compare=glb:out/plain.glb,glb:out/applied.glb glb_structure
import bpy
bpy.data.objects['Cube'].hide_render = True
bpy.ops.object.armature_add(location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones.new('Top')
bpy.data.armatures['Armature'].edit_bones['Top'].head = (0, 0, 1.0)
bpy.data.armatures['Armature'].edit_bones['Top'].tail = (0, 0, 2.0)
bpy.data.armatures['Armature'].edit_bones['Top'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Armature'].pose.bones['Top'].rotation_quaternion = (0.9, 0.0, 0.4, 0.0)
bpy.ops.mesh.primitive_cylinder_add(vertices=6, radius=0.4, depth=2.0, location=(0.0, 0.0, 1.0), rotation=(0, 0, 0.4), scale=(1.0, 1.5, 1.0))
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
bpy.data.objects['Cylinder'].data.shape_keys.key_blocks['Bulge'].data[3].co = (-0.4, 0.7, 1.2)
bpy.data.objects['Cylinder'].data.shape_keys.key_blocks['Bulge'].value = 0.5
bpy.ops.mesh.primitive_cube_add(size=1.0, location=(2.0, 0.0, 0.5))
bpy.data.objects['Cube.001'].vertex_groups.new(name='Bone')
bpy.data.objects['Cube.001'].vertex_groups['Bone'].add([0, 1, 2, 3, 4, 5, 6, 7], 1.0, 'REPLACE')
bpy.data.objects['Cube.001'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube.001'].modifiers['Subdivision'].levels = 1
bpy.data.objects['Cube.001'].modifiers.new(name='Armature', type='ARMATURE')
bpy.data.objects['Cube.001'].modifiers['Armature'].object = bpy.data.objects['Armature']
bpy.data.objects['Cube.001'].parent = bpy.data.objects['Armature']
bpy.data.objects['Cube.001'].shape_key_add(name='Basis')
bpy.data.objects['Cube.001'].shape_key_add(name='Lift')
bpy.data.objects['Cube.001'].data.shape_keys.key_blocks['Lift'].data[0].co = (-0.5, -0.5, 0.0)
bpy.ops.export_scene.gltf(filepath='out/plain.glb')
bpy.ops.export_scene.gltf(filepath='out/applied.glb', export_apply=True)
