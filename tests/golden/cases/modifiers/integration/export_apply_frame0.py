# M3+M4 stabilization round 2: the exporter evaluates at frame 0 unless export_current_frame
# (`export.save`: `scene.frame_set(0)`, restored afterwards). BUILD (frame_start=-20,
# frame_duration=50) after SUBSURF keeps 9 of 24 faces at frame 0 and 10 at frame 1.
# golden: startup=factory tier=T2 compare=glb:out/frame0.glb,glb:out/current.glb,context
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube'].modifiers.new(name='Build', type='BUILD')
bpy.data.objects['Cube'].modifiers['Build'].frame_start = -20
bpy.data.objects['Cube'].modifiers['Build'].frame_duration = 50
bpy.ops.export_scene.gltf(filepath='out/frame0.glb', export_apply=True)
bpy.ops.export_scene.gltf(filepath='out/current.glb', export_apply=True, export_current_frame=True)
