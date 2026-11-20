# WP 6.7: animated parents and children. Sampled channels are each object's matrix relative to
# its exported parent at every frame; F-curve channels are transformed by
# matrix_parent_inverse (the add-on's non-sampled rule). Rotation modes: an Euler XZY object
# is sampled as quaternions; with export_force_sampling=False Euler curves become quaternion
# CUBICSPLINE keys (the add-on reads them as XYZ), a quaternion object keeps its keys. A
# filtered-out parent (use_selection) makes the child relative to the root.
# golden: startup=factory tier=T2 tol=1e-5 compare=glb:out/sampled.glb,glb:out/curves.glb,glb:out/selected.glb
import bpy
bpy.ops.object.empty_add(location=(1.0, 1.0, 0.0))
bpy.data.objects['Empty'].name = 'Parent'
bpy.data.objects['Parent'].rotation_mode = 'XZY'
bpy.data.objects['Parent'].keyframe_insert('rotation_euler', frame=1)
bpy.data.objects['Parent'].rotation_euler = (0.4, 0.0, 1.2)
bpy.data.objects['Parent'].keyframe_insert('rotation_euler', frame=7)
bpy.data.objects['Parent'].keyframe_insert('location', frame=1)
bpy.data.objects['Parent'].location = (2.0, 1.0, 1.0)
bpy.data.objects['Parent'].keyframe_insert('location', frame=7)
bpy.data.objects['Cube'].location = (0.0, 0.0, 2.0)
bpy.data.objects['Cube'].parent = bpy.data.objects['Parent']
bpy.data.objects['Cube'].rotation_mode = 'QUATERNION'
bpy.data.objects['Cube'].keyframe_insert('rotation_quaternion', frame=2)
bpy.data.objects['Cube'].rotation_quaternion = (0.8660254, 0.5, 0.0, 0.0)
bpy.data.objects['Cube'].keyframe_insert('rotation_quaternion', frame=6)
bpy.data.objects['Cube'].keyframe_insert('scale', frame=2)
bpy.data.objects['Cube'].scale = (0.5, 0.5, 2.0)
bpy.data.objects['Cube'].keyframe_insert('scale', frame=6)
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.ops.export_scene.gltf(filepath='out/sampled.glb')
bpy.ops.export_scene.gltf(filepath='out/curves.glb', export_force_sampling=False)
bpy.ops.export_scene.gltf(filepath='out/selected.glb', use_selection=True)
