# WP 6.7: a keyed but constant channel of a child whose parent is animated. Sampled, the
# child's matrix relative to its parent differs by float rounding between frames; the add-on
# keeps the first and last keys of a constant channel (np.ptp < 1e-4) and writes STEP when
# keyframes[0].value == keyframes[1].value, a mathutils comparison that accepts one ULP per
# component (EXPP_VectorsAreEqual(..., 1)).
# golden: startup=factory tier=T2 tol=1e-5 compare=glb:out/sampled.glb glb_structure
import bpy
bpy.data.objects['Cube'].keyframe_insert('rotation_euler', frame=1)
bpy.data.objects['Cube'].rotation_euler = (0.3, 0.2, 1.5)
bpy.data.objects['Cube'].keyframe_insert('rotation_euler', frame=10)
bpy.context.scene.frame_set(1)
bpy.data.objects['Camera'].parent = bpy.data.objects['Cube']
bpy.data.objects['Camera'].keyframe_insert('rotation_euler', frame=1)
bpy.data.objects['Camera'].keyframe_insert('rotation_euler', frame=10)
bpy.context.scene.frame_set(1)
bpy.ops.export_scene.gltf(filepath='out/sampled.glb', export_cameras=True)
