# WP 3.2: `bpy.ops.image.new` (names, colour depth, fill colour, float fill, the UV_GRID and
# COLOR_GRID patterns, tiled images) and the context-image operators, whose poll fails headless
# (no Image Editor). Values are copied into empties' transforms so the objects comparator checks
# them; names are copied into object names.
# golden: startup=factory tier=T0 compare=objects,context
import bpy
bpy.ops.image.new()
bpy.ops.image.new()
bpy.ops.image.new(name='Grey', width=2, height=1, color=(0.5, 0.5, 0.5, 0.5))
bpy.ops.image.new(name='Opaque', width=2, height=1, color=(0.5, 0.25, 0.1, 0.5), alpha=False)
bpy.ops.image.new(name='Float', width=2, height=1, color=(0.5, 0.25, 0.1, 0.5), float=True)
bpy.ops.image.new(name='Grid', width=40, height=36, generated_type='UV_GRID')
bpy.ops.image.new(name='Colors', width=30, height=40, generated_type='COLOR_GRID')
bpy.ops.image.new(name='Tiles', width=8, height=8, tiled=True)
bpy.ops.image.new(name='Zero', width=0, height=2)
bpy.data.objects.new('Untitled', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Untitled'])
bpy.data.objects['Untitled'].location.x = bpy.data.images['Untitled'].size[0]
bpy.data.objects['Untitled'].location.y = bpy.data.images['Untitled'].size[1]
bpy.data.objects['Untitled'].location.z = bpy.data.images['Untitled'].depth
bpy.data.objects['Untitled'].scale = bpy.data.images['Untitled.001'].generated_color[0:3]
bpy.data.objects.new('Grey', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Grey'])
bpy.data.objects['Grey'].location = bpy.data.images['Grey'].pixels[0:3]
bpy.data.objects['Grey'].scale = bpy.data.images['Grey'].pixels[1:4]
bpy.data.objects['Grey'].rotation_euler = bpy.data.images['Grey'].generated_color[1:4]
bpy.data.objects.new('Opaque', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Opaque'])
bpy.data.objects['Opaque'].location = bpy.data.images['Opaque'].pixels[1:4]
bpy.data.objects['Opaque'].scale = bpy.data.images['Opaque'].generated_color[1:4]
bpy.data.objects['Opaque'].rotation_euler.x = bpy.data.images['Opaque'].depth
bpy.data.objects.new('Float', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Float'])
bpy.data.objects['Float'].location = bpy.data.images['Float'].pixels[0:3]
bpy.data.objects['Float'].scale = bpy.data.images['Float'].pixels[1:4]
bpy.data.objects['Float'].rotation_euler.x = bpy.data.images['Float'].depth
bpy.data.objects['Float'].rotation_euler.y = bpy.data.images['Float'].is_float
bpy.data.objects.new('Grid', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Grid'])
bpy.data.objects['Grid'].location = bpy.data.images['Grid'].pixels[0:3]
bpy.data.objects['Grid'].scale = bpy.data.images['Grid'].pixels[200:203]
bpy.data.objects['Grid'].rotation_euler = bpy.data.images['Grid'].pixels[2624:2627]
bpy.data.objects.new('Grid2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Grid2'])
bpy.data.objects['Grid2'].location = bpy.data.images['Grid'].pixels[5300:5303]
bpy.data.objects['Grid2'].scale = bpy.data.images['Grid'].pixels[2944:2947]
bpy.data.objects.new('Colors', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Colors'])
bpy.data.objects['Colors'].location = bpy.data.images['Colors'].pixels[0:3]
bpy.data.objects['Colors'].scale = bpy.data.images['Colors'].pixels[2524:2527]
bpy.data.objects['Colors'].rotation_euler = bpy.data.images['Colors'].pixels[4796:4799]
bpy.data.objects.new('Tiles', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Tiles'])
bpy.data.objects['Tiles'].location.x = bpy.data.images['Tiles'].size[0]
bpy.data.objects['Tiles'].location.y = bpy.data.images['Tiles'].depth
bpy.data.objects['Tiles'].name = bpy.data.images['Tiles'].source
bpy.data.objects['Grid'].name = bpy.data.images['Grid'].type
bpy.data.objects['Colors'].name = bpy.data.images['Colors'].generated_type
bpy.data.objects.new('Zero', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Zero'])
bpy.data.objects['Zero'].location.x = bpy.data.images['Zero'].size[0]
bpy.data.objects['Zero'].location.y = bpy.data.images['Zero'].users
bpy.data.objects['Zero'].name = bpy.data.images['Float'].colorspace_settings.name
bpy.ops.image.pack()
bpy.ops.image.unpack(id='Grey')
bpy.ops.image.save()
bpy.ops.image.save_as(filepath='out/x.png')
bpy.ops.image.reload()
bpy.ops.image.new(name='Bad', generated_type='STRIPES')
