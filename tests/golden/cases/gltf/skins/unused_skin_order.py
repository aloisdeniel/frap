# M5+M6 stabilization round 6: the skin of an armature that deforms no exported mesh is
# gathered with its scene but written after the animations (`traverse_unused_skins`), so its
# inverse-bind-matrix accessor comes last. Two armatures (the default cube is exported too),
# the second one's pose bone keyed: the skins' matrices are accessors 9 and 10, after the
# animation's, as in Blender 5.2.4 (frap wrote them right after the meshes, shifting every
# animation accessor index). The canonical comparison does not see accessor indices; they are
# checked by `frap-ops-io-tests` (`gltf_animation_structure`, skins).
# golden: startup=factory order=canonical tier=T2 tol=1e-5 compare=glb:out/unused.glb glb_structure
import bpy
bpy.ops.object.armature_add()
bpy.ops.object.armature_add(location=(2.0, 0.0, 0.0))
bpy.data.objects['Armature.001'].pose.bones['Bone'].keyframe_insert('location', frame=1)
bpy.data.objects['Armature.001'].pose.bones['Bone'].location = (0.0, 1.0, 0.0)
bpy.data.objects['Armature.001'].pose.bones['Bone'].keyframe_insert('location', frame=10)
bpy.ops.export_scene.gltf(filepath='out/unused.glb')
