# M5+M6 stabilization round 3: KHR_animation_pointer in SCENE mode next to object channels
# sampled over the same frames. The add-on's `@cached gather_accessor` is export-wide, so the
# light energy channel reuses the object channels' time accessor (Blender 5.2.4: 17 accessors,
# 18 buffer views); the `structure` entry of the canonical form compares those counts.
# golden: startup=factory tier=T2 compare=glb:out/scene_shared_input.glb glb_structure
import bpy
bpy.context.scene.frame_end = 30
bpy.data.objects['Cube'].location = (0.0, 0.0, 0.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=1)
bpy.data.objects['Cube'].location = (0.0, 0.0, 2.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=30)
bpy.data.lights['Light'].energy = 100.0
bpy.data.lights['Light'].keyframe_insert('energy', frame=1)
bpy.data.lights['Light'].energy = 1000.0
bpy.data.lights['Light'].keyframe_insert('energy', frame=30)
bpy.ops.export_scene.gltf(filepath='out/scene_shared_input.glb', export_animation_mode='SCENE', export_pointer_animation=True, export_lights=True)
