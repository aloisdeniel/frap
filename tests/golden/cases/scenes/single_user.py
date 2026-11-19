# WP 3.10 (M34W0-4): `object.make_single_user`. `object=True` copies the objects that are in
# two scenes (a linked copy) and relinks the scene and its collections without copying them,
# so the shared collection holds the copies in both scenes; `obdata=True` and `material=True`
# give each selected object its own data and materials in object order (the last user, an
# orphan object here, keeps the original); `animation=True` with nothing shared changes
# nothing. Poll: Object Mode.
# golden: startup=factory tier=T0 compare=objects,materials,context
import bpy
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.ops.scene.new(type='LINK_COPY')
bpy.ops.object.make_single_user(type='ALL', object=True)
bpy.context.scene.camera.name
bpy.context.view_layer.objects.active.name
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.scene.camera.name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.view_layer.objects.active.name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.objects['Light.001'].parent.name, None))
bpy.data.scenes['Scene'].collection.objects.link(bpy.data.objects.new('H1', None))
bpy.data.objects['H1'].location = (bpy.data.objects['Cube'].users, bpy.data.objects['Cube.001'].users, bpy.data.meshes['Cube'].users)
bpy.context.window.scene = bpy.data.scenes['Scene']
bpy.context.scene.camera.name
bpy.context.view_layer.objects.active
bpy.ops.object.duplicate(linked=True)
bpy.ops.object.duplicate(linked=True)
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.make_single_user(obdata=True, material=True, animation=True, obdata_animation=True)
bpy.ops.object.make_single_user(type='ALL', obdata=True, material=True)
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.objects['Cube.002'].data.name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.objects['Cube.003'].data.name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.objects['Light.001'].data.name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.objects['Camera.001'].data.name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new('H2', None))
bpy.data.objects['H2'].scale = (bpy.data.meshes['Cube'].users, bpy.data.materials['Material'].users, bpy.data.materials['Material.003'].users)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.make_single_user(obdata=True)
bpy.ops.object.mode_set(mode='OBJECT')
