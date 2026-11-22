# M7 stabilization: Blender-written meshopt files with animation import in frap as in Blender.
# The inputs (`import_blender_anim.inputs/`, written by `tests/cli/m7/meshopt/blender_meshopt.py
# anim` with Blender 5.2.4) hold the factory cube with keyed location, rotation_euler and scale
# and a keyed shape key, and an armature with a keyed pose bone, exported with
# export_meshopt_compression_enable: anim_ext.glb (EXT) and anim_khr_sep.gltf (KHR,
# GLTF_SEPARATE) through the add-on's sampled paths (TIME, location and scale EXPONENTIAL, the
# object rotation QUATERNION at byteStride 16, the bone rotation at 8, the weights SK_ANIM),
# anim_curves.glb (EXT) through the fcurves path (export_force_sampling=False: CUBICSPLINE,
# rotations at byteStride 8, no shape-key animation, on which that path raises in Blender).
# golden: startup=empty tier=T2 tol=1e-5 compare=objects,anim:CubeAction,anim:CubeAction.001,anim:CubeAction.002,anim:RigAction,anim:RigAction.001,anim:RigAction.002
import bpy
bpy.ops.import_scene.gltf(filepath='anim_ext.glb')
bpy.ops.import_scene.gltf(filepath='anim_khr_sep.gltf')
bpy.ops.import_scene.gltf(filepath='anim_curves.glb')
