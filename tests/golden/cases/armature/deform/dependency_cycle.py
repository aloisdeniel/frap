# WP 6.5 fix: an armature parented (OBJECT) to the mesh it deforms is no dependency cycle: the
# armature needs the mesh's transform only (a vertex parent, a real cycle whose result depends on
# Blender's evaluation order, is covered by a frap-eval unit test). A SIMPLE_DEFORM origin that
# is a child of the deformed mesh reads the mesh's transform the same way.
# golden: startup=factory tier=T1 tol=1e-5 compare=objects,evaluated:Cube,evaluated:Cube.001
import bpy
bpy.ops.object.armature_add(location=(0, 0, -1))
bpy.data.objects['Cube'].vertex_groups.new(name='Bone')
bpy.data.objects['Cube'].vertex_groups['Bone'].add([0, 1, 2, 3], 1.0, 'REPLACE')
bpy.data.objects['Cube'].modifiers.new('Rig', 'ARMATURE')
bpy.data.objects['Cube'].modifiers['Rig'].object = bpy.data.objects['Armature']
bpy.data.objects['Cube'].location = (0.5, 0.25, 0.0)
bpy.data.objects['Armature'].parent = bpy.data.objects['Cube']
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.9, 0.2, 0.3, 0.0)
bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.vertices[0].co
bpy.data.objects['Armature'].matrix_world.translation
bpy.ops.mesh.primitive_cube_add(size=1.0, location=(3, 0, 0))
bpy.data.objects.new('E', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['E'])
bpy.data.objects['E'].location = (0.5, 0.0, 0.0)
bpy.data.objects['Cube.001'].modifiers.new('SD', 'SIMPLE_DEFORM')
bpy.data.objects['Cube.001'].modifiers['SD'].origin = bpy.data.objects['E']
bpy.data.objects['E'].parent = bpy.data.objects['Cube.001']
bpy.data.objects['Cube.001'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.vertices[0].co
