# M3+M4 stabilization round 5 (M34S5-1): export_apply=True with objects linked into three
# scenes at different frames. The add-on gathers each scene with `bpy.context.window.scene` set
# to it, so a shared object with a frame-dependent modifier (Build) is evaluated in each scene's
# own depsgraph at that scene's frame, and each scene's node gets its own glTF mesh. Exported
# from each of the three window scenes (`frame_set(0)` only moves the window's scene).
# golden: startup=factory tier=T2 compare=glb:out/from_scene.glb,glb:out/from_s2.glb,glb:out/from_s3.glb,context
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Build', type='BUILD')
bpy.data.objects['Cube'].modifiers['Build'].frame_start = -5
bpy.data.objects['Cube'].modifiers['Build'].frame_duration = 20
bpy.ops.mesh.primitive_monkey_add(location=(3, 0, 0))
bpy.context.object.modifiers.new(name='Build', type='BUILD')
bpy.context.object.modifiers['Build'].frame_start = -5
bpy.context.object.modifiers['Build'].frame_duration = 20
bpy.data.scenes.new('S2')
bpy.data.scenes['S2'].frame_current = 9
bpy.data.scenes['S2'].collection.objects.link(bpy.data.objects['Cube'])
bpy.data.scenes['S2'].collection.objects.link(bpy.data.objects['Suzanne'])
bpy.data.scenes.new('S3')
bpy.data.scenes['S3'].frame_current = 13
bpy.data.scenes['S3'].collection.objects.link(bpy.data.objects['Cube'])
bpy.data.scenes['S3'].collection.objects.link(bpy.data.objects['Suzanne'])
bpy.ops.export_scene.gltf(filepath='out/from_scene.glb', export_apply=True)
bpy.context.window.scene = bpy.data.scenes['S2']
bpy.ops.export_scene.gltf(filepath='out/from_s2.glb', export_apply=True)
bpy.context.window.scene = bpy.data.scenes['S3']
bpy.ops.export_scene.gltf(filepath='out/from_s3.glb', export_apply=True, export_current_frame=True)
bpy.context.window.scene = bpy.data.scenes['Scene']
