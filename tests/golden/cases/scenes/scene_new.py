# WP 3.10: `scene.new(type=…)` makes the new scene the window's scene. `NEW` is a default scene
# named `Scene.NNN`; `EMPTY` copies the settings (render, frame range and current frame, units)
# but not the cursor, camera, world or objects; `LINK_COPY` shares collections, objects, camera
# and world and keeps the selection and active object; `FULL_COPY` copies the world, the
# collections, the objects and their data (materials shared), relinks parents and the camera
# to the copies and keeps the selection and active object. Values of each new scene are read
# back into empties linked to the original scene, which is shown again at the end.
# golden: startup=factory tier=T0 compare=objects,materials,context
import bpy
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Cube'].location = (1, 0, 0)
bpy.data.scenes['Scene'].render.resolution_x = 640
bpy.data.scenes['Scene'].frame_end = 100
bpy.data.scenes['Scene'].frame_current = 7
bpy.data.scenes['Scene'].cursor.location = (1, 2, 3)
bpy.data.scenes['Scene'].unit_settings.system = 'IMPERIAL'
bpy.data.scenes['Scene'].render.filepath = 'out/x'
bpy.ops.scene.new(type='NEW')
bpy.data.scenes['Scene'].collection.objects.link(bpy.data.objects.new(bpy.context.scene.name, None))
bpy.data.objects['Scene.001'].location = (bpy.context.scene.render.resolution_x, bpy.context.scene.frame_end, bpy.context.scene.frame_current)
bpy.data.objects['Scene.001'].scale = bpy.context.scene.cursor.location
bpy.data.scenes['Scene'].collection.objects.link(bpy.data.objects.new(bpy.context.scene.unit_settings.system, None))
bpy.context.window.scene = bpy.data.scenes['Scene']
bpy.ops.scene.new(type='EMPTY')
bpy.data.scenes['Scene'].collection.objects.link(bpy.data.objects.new(bpy.context.scene.name, None))
bpy.data.objects['Scene.002'].location = (bpy.context.scene.render.resolution_x, bpy.context.scene.frame_end, bpy.context.scene.frame_current)
bpy.data.objects['Scene.002'].scale = bpy.context.scene.cursor.location
bpy.data.scenes['Scene'].collection.objects.link(bpy.data.objects.new(bpy.context.scene.unit_settings.system, None))
bpy.data.scenes['Scene'].collection.objects.link(bpy.data.objects.new(bpy.context.scene.render.filepath, None))
bpy.context.scene.camera
bpy.context.scene.world
bpy.context.view_layer.objects.active
bpy.context.window.scene = bpy.data.scenes['Scene']
bpy.ops.scene.new(type='LINK_COPY')
bpy.data.scenes['Scene'].collection.objects.link(bpy.data.objects.new(bpy.context.scene.name, None))
bpy.data.scenes['Scene'].collection.objects.link(bpy.data.objects.new(bpy.context.view_layer.objects.active.name, None))
bpy.data.scenes['Scene'].collection.objects.link(bpy.data.objects.new(bpy.context.scene.camera.name, None))
bpy.data.scenes['Scene'].collection.objects.link(bpy.data.objects.new(bpy.context.scene.world.name, None))
bpy.data.scenes['Scene'].collection.objects.link(bpy.data.objects.new(bpy.context.collection.name, None))
bpy.data.objects['Scene.003'].location = (bpy.context.scene.render.resolution_x, bpy.context.scene.frame_end, bpy.context.scene.frame_current)
bpy.data.objects['Scene.003'].scale = bpy.context.scene.cursor.location
bpy.data.scenes['Scene.003'].objects['Light']
bpy.context.window.scene = bpy.data.scenes['Scene']
bpy.ops.scene.new(type='FULL_COPY')
bpy.data.scenes['Scene'].collection.objects.link(bpy.data.objects.new(bpy.context.scene.name, None))
bpy.data.scenes['Scene'].collection.objects.link(bpy.data.objects.new(bpy.context.view_layer.objects.active.name, None))
bpy.data.scenes['Scene'].collection.objects.link(bpy.data.objects.new(bpy.context.scene.camera.name, None))
bpy.data.scenes['Scene'].collection.objects.link(bpy.data.objects.new(bpy.context.scene.world.name, None))
bpy.data.scenes['Scene'].collection.objects.link(bpy.data.objects.new(bpy.context.collection.name, None))
bpy.data.scenes['Scene'].collection.objects.link(bpy.data.objects.new(bpy.context.selected_objects[0].name, None))
bpy.data.scenes['Scene'].collection.objects.link(bpy.data.objects.new(bpy.data.objects['Light.001'].parent.name, None))
bpy.data.scenes['Scene'].collection.objects.link(bpy.data.objects.new(bpy.data.objects['Cube.001'].data.name, None))
bpy.data.objects['Scene.004'].location = (bpy.context.scene.render.resolution_x, bpy.context.scene.frame_end, bpy.context.scene.frame_current)
bpy.data.objects['Scene.004'].scale = bpy.context.scene.cursor.location
bpy.data.objects['Collection.001'].location = (bpy.data.worlds['World'].users, bpy.data.worlds['World.001'].users, bpy.data.collections['Collection'].users)
bpy.data.objects['Collection.001'].scale = (bpy.data.materials['Material'].users, bpy.data.meshes['Cube'].users, bpy.data.cameras['Camera.001'].users)
bpy.ops.scene.new(type='COPY')
bpy.ops.scene.new(type='FULL_COPY')
bpy.context.scene.name
bpy.context.window.scene = bpy.data.scenes['Scene']
