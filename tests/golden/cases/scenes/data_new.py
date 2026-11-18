# WP 3.10: `bpy.data.scenes.new(name)`: names are uniquified ('' gives `Scene`), the window
# keeps its scene, a new scene has no objects, camera or world and Blender's default settings
# (not the factory scene's: `render.filepath`, the Workbench shading). Values are read back
# into the names and transforms of empties linked to the context scene, so the dump compares
# them.
# golden: startup=factory tier=T0 compare=objects,context
import bpy
bpy.data.scenes.new('S2')
bpy.data.scenes.new('S2')
bpy.data.scenes.new('')
bpy.data.scenes.new(None)
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.scene.name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.scenes[0].name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.scenes[1].name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.scenes[2].name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.scenes[3].name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.scenes['S2'].render.filepath, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.scenes['S2'].view_layers[0].name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.scenes['S2'].render.engine, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.scenes['S2'].view_settings.view_transform, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.scenes['S2'].display.shading.cavity_type, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.scenes['S2'].view_layers['ViewLayer'].active_layer_collection.name, None))
bpy.data.objects['S2'].location = (bpy.data.scenes['S2'].frame_start, bpy.data.scenes['S2'].frame_end, bpy.data.scenes['S2'].frame_current)
bpy.data.objects['S2.001'].location = (bpy.data.scenes['S2'].render.resolution_x, bpy.data.scenes['S2'].render.resolution_y, bpy.data.scenes['S2'].render.fps)
bpy.data.objects['Scene.001'].location = bpy.data.scenes['S2'].display.shading.background_color
bpy.data.objects['Scene.002'].location = (bpy.data.scenes['S2'].display.shading.xray_alpha_wireframe, bpy.data.scenes['S2'].display.shading.studiolight_intensity, bpy.data.scenes['S2'].unit_settings.scale_length)
bpy.data.objects['ViewLayer'].location = bpy.data.scenes['S2'].cursor.location
bpy.data.objects['ViewLayer'].scale = (bpy.data.scenes['S2'].users, bpy.data.scenes['Scene'].users, bpy.data.scenes['Scene.001'].users)
bpy.data.objects['Scene'].location = (bpy.data.scenes['S2'].eevee.taa_render_samples, bpy.data.scenes['S2'].render.resolution_percentage, bpy.data.scenes['S2'].frame_step)
bpy.data.scenes['S2'].camera
bpy.data.scenes['S2'].world
bpy.data.scenes['S2'].view_layers['ViewLayer'].objects.active
bpy.data.scenes['S2'].objects['Cube']
bpy.data.scenes['S2'].collection.children['Collection']
bpy.data.scenes.new(name='S3', x=1)
bpy.data.scenes.new()
