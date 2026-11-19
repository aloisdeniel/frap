# WP 3.10: scenes round-trip through save_as_mainfile / open_mainfile: every scene with its
# settings, objects, shared (linked copy) and copied (full copy) collections, camera, world,
# selection and active object, and the window's scene and view layer.
# Paths are in the work directory: Blender's save does not create directories.
# golden: startup=factory tier=T1 compare=objects,materials,context tol=1e-6
import bpy
bpy.ops.mesh.primitive_cone_add(vertices=7, location=(2, 0, 0))
bpy.ops.scene.new(type='LINK_COPY')
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.frame_end = 42
bpy.ops.scene.new(type='FULL_COPY')
bpy.data.objects['Cone.001'].location = (0, 4, 0)
bpy.data.objects['Cube.001'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.001']
bpy.data.scenes.new('Empty')
bpy.context.window.scene = bpy.data.scenes['Scene.001']
bpy.ops.wm.save_as_mainfile(filepath='s.frap')
bpy.data.scenes.remove(bpy.data.scenes['Scene.002'])
bpy.context.window.scene = bpy.data.scenes['Scene']
bpy.ops.wm.open_mainfile(filepath='s.frap')
bpy.context.scene.name
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.scene.name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.scenes['Scene.002'].camera.name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.scenes['Scene.002'].world.name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.scenes['Scene.002'].view_layers[0].objects.active.name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.scenes['Empty'].render.filepath, None))
bpy.data.objects['Scene.001'].location = (bpy.data.scenes['Scene.001'].render.resolution_x, bpy.data.scenes['Scene.001'].frame_end, bpy.data.scenes['Scene.002'].render.resolution_x)
bpy.data.objects['Scene.001'].scale = (bpy.data.collections['Collection'].users, bpy.data.collections['Collection.001'].users, bpy.data.scenes['Empty'].display.shading.xray_alpha_wireframe)
bpy.data.scenes['Scene.002'].objects['Cone.001']
bpy.data.scenes['Empty'].objects['Cone']
bpy.context.view_layer.update()
bpy.context.window.scene = bpy.data.scenes['Scene.002']
bpy.context.view_layer.update()
