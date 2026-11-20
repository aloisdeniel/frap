# golden: startup=factory compare=context tier=T0
# render-animation: files needs=animation images=out/anim_0001.png,out/anim_0003.png,out/anim_0005.png,out/still.png
# render.render(animation=True) of an animated scene (WP 6.6, ARCHITECTURE §19.4): each frame
# renders from its own copy of the state with the animation of that frame (the Cube's location
# and rotation, the Camera's lens), frames 1, 3 and 5 (scene frame_step 2), then a still at
# frame_current. Each image is compared with Blender's render (SSIM, Workbench). frame_current
# is unchanged afterwards (cursor). Originals of animated channels are not dumped after the
# render (D-07). `#@render` lines run in crates/frap-ops-render-tests (tests/render_animation.rs).
import bpy
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 96
bpy.context.scene.render.resolution_y = 54
bpy.context.scene.frame_start = 1
bpy.context.scene.frame_end = 5
bpy.context.scene.frame_step = 2
bpy.context.scene.frame_current = 4
bpy.data.actions.new('CubeMove')
bpy.data.actions['CubeMove'].slots.new(id_type='OBJECT', name='Cube')
bpy.data.actions['CubeMove'].layers.new('Layer')
bpy.data.actions['CubeMove'].layers[0].strips.new(type='KEYFRAME')
bpy.data.actions['CubeMove'].layers[0].strips[0].channelbag(bpy.data.actions['CubeMove'].slots[0], ensure=True)
bpy.data.actions['CubeMove'].layers[0].strips[0].channelbags[0].fcurves.new('location', index=0)
bpy.data.actions['CubeMove'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(1, -3.0)
bpy.data.actions['CubeMove'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(5, 3.0)
bpy.data.actions['CubeMove'].layers[0].strips[0].channelbags[0].fcurves.new('rotation_euler', index=2)
bpy.data.actions['CubeMove'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.insert(1, 0.0)
bpy.data.actions['CubeMove'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.insert(5, 1.5)
bpy.data.objects['Cube'].animation_data_create()
bpy.data.objects['Cube'].animation_data.action = bpy.data.actions['CubeMove']
bpy.data.objects['Cube'].animation_data.action_slot = bpy.data.actions['CubeMove'].slots[0]
bpy.data.actions.new('Zoom')
bpy.data.actions['Zoom'].slots.new(id_type='CAMERA', name='Camera')
bpy.data.actions['Zoom'].layers.new('Layer')
bpy.data.actions['Zoom'].layers[0].strips.new(type='KEYFRAME')
bpy.data.actions['Zoom'].layers[0].strips[0].channelbag(bpy.data.actions['Zoom'].slots[0], ensure=True)
bpy.data.actions['Zoom'].layers[0].strips[0].channelbags[0].fcurves.new('lens', index=0)
bpy.data.actions['Zoom'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(1, 30.0)
bpy.data.actions['Zoom'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(5, 60.0)
bpy.data.cameras['Camera'].animation_data_create()
bpy.data.cameras['Camera'].animation_data.action = bpy.data.actions['Zoom']
bpy.data.cameras['Camera'].animation_data.action_slot = bpy.data.actions['Zoom'].slots[0]
bpy.context.scene.render.filepath = 'out/anim_'
#@render bpy.ops.render.render(animation=True)
bpy.context.scene.render.filepath = 'out/still'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.cursor.location = (bpy.context.scene.frame_current, bpy.context.scene.frame_start, bpy.context.scene.frame_end)
