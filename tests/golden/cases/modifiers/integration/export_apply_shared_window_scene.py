# M3+M4 stabilization round 5 (M34S5-1): a Cube with a Build linked into a second scene, exported
# with export_apply=True from either window scene. The window's scene is evaluated at frame 0
# (`frame_set(0)`: before the Build starts, so its node has no mesh) and the other scene at its
# own frame, each in its own depsgraph: the geometry of the non-window scene is not lost.
# golden: startup=factory tier=T2 compare=glb:out/a.glb,glb:out/b.glb,context glb_structure
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Build', type='BUILD')
bpy.data.objects['Cube'].modifiers['Build'].frame_duration = 10
bpy.context.scene.frame_current = 4
bpy.data.scenes.new('S2')
bpy.data.scenes['S2'].frame_current = 8
bpy.data.scenes['S2'].collection.objects.link(bpy.data.objects['Cube'])
bpy.ops.export_scene.gltf(filepath='out/a.glb', export_apply=True, export_animations=False)
bpy.context.window.scene = bpy.data.scenes['S2']
bpy.ops.export_scene.gltf(filepath='out/b.glb', export_apply=True, export_animations=False)
