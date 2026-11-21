# M5+M6 stabilization round 5: two armatures (one rotated) and two cubes skinned to them, one
# rotated relative to its armature, exported without animations. A skinned mesh's split key
# holds its normals moved by `normal_transform` (the add-on transforms them before filling
# its dots), so the corners sort and deduplicate as in Blender (the rotated cube got its own
# index accessor: 18 accessors), and the written normals fold `-0.0` into `0.0` as the dots
# do (byte-identical BIN chunk on 5.2.4; the canonical comparison ignores the sign of zero).
# golden: startup=factory order=canonical tier=T2 tol=1e-5 compare=glb:out/skinned.glb glb_structure
import bpy
bpy.ops.object.armature_add(location=(0.0, 0.0, 0.0))
bpy.ops.object.armature_add(location=(3.0, 0.0, 0.0))
bpy.data.objects['Armature.001'].rotation_euler = (0.3, 0.0, 1.2)
bpy.ops.mesh.primitive_cube_add(location=(0.0, 0.0, 2.0))
bpy.ops.mesh.primitive_cube_add(location=(3.0, 1.0, 0.0))
bpy.data.objects['Cube.002'].rotation_euler = (0.0, 0.5, 0.0)
bpy.data.objects['Cube.001'].modifiers.new('Arm', 'ARMATURE')
bpy.data.objects['Cube.001'].modifiers['Arm'].object = bpy.data.objects['Armature']
bpy.data.objects['Cube.001'].vertex_groups.new(name='Bone')
bpy.data.objects['Cube.001'].vertex_groups['Bone'].add([0, 1, 2, 3, 4, 5, 6, 7], 1.0, 'REPLACE')
bpy.data.objects['Cube.002'].modifiers.new('Arm', 'ARMATURE')
bpy.data.objects['Cube.002'].modifiers['Arm'].object = bpy.data.objects['Armature.001']
bpy.data.objects['Cube.002'].vertex_groups.new(name='Bone')
bpy.data.objects['Cube.002'].vertex_groups['Bone'].add([0, 1, 2, 3, 4, 5, 6, 7], 1.0, 'REPLACE')
bpy.ops.export_scene.gltf(filepath='out/skinned.glb', export_animations=False)
