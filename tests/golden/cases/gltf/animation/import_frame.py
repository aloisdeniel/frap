# WP 6.7: after import_scene.gltf the restored first animation is already applied at
# scene.frame_current (the new IDs are tagged and bpy.ops updates the view layer after the
# FINISHED call), with constant extrapolation past the last key, so the imported objects do
# not keep their rest transforms. Probe holds the values seen right after the first import
# (frame 6); the second import (frame 30) is past the end.
# golden: startup=factory tier=T1 tol=1e-5 compare=objects
import bpy
bpy.data.objects['Cube'].rotation_mode = 'QUATERNION'
bpy.data.objects['Cube'].keyframe_insert('rotation_quaternion', frame=1)
bpy.data.objects['Cube'].keyframe_insert('location', frame=1)
bpy.data.objects['Cube'].rotation_quaternion = (0.7071068, 0.0, 0.0, 0.7071068)
bpy.data.objects['Cube'].location = (2.0, 0.0, 1.0)
bpy.data.objects['Cube'].keyframe_insert('rotation_quaternion', frame=11)
bpy.data.objects['Cube'].keyframe_insert('location', frame=11)
bpy.ops.export_scene.gltf(filepath='out/anim.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.context.scene.frame_current = 6
bpy.ops.import_scene.gltf(filepath='out/anim.glb')
bpy.context.scene.collection.objects.link(bpy.data.objects.new('Probe', None))
bpy.data.objects['Probe'].location = bpy.data.objects['Cube'].location
bpy.data.objects['Probe'].rotation_mode = 'QUATERNION'
bpy.data.objects['Probe'].rotation_quaternion = bpy.data.objects['Cube'].rotation_quaternion
bpy.context.scene.frame_current = 30
bpy.ops.import_scene.gltf(filepath='out/anim.glb')
