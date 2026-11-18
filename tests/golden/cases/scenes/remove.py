# WP 3.10: `bpy.data.scenes.remove` and `scene.delete`: the last local scene cannot be removed
# (and `scene.delete` does not poll), `do_unlink=False` always refuses (every scene has a user),
# removing the window's scene shows the previous scene of `bpy.data.scenes` (else the next),
# and the removed scene's objects stay in `bpy.data`.
# golden: startup=factory tier=T0 compare=objects,context
import bpy
bpy.ops.scene.delete()
bpy.data.scenes.remove(bpy.data.scenes['Scene'])
bpy.data.scenes.remove(bpy.data.scenes['Scene'], do_unlink=False)
bpy.data.scenes.new('A')
bpy.data.scenes.new('B')
bpy.data.scenes.new('C')
bpy.data.scenes.remove(bpy.data.scenes['A'], do_unlink=False)
bpy.data.scenes.remove(bpy.data.scenes['B'])
bpy.context.window.scene = bpy.data.scenes['C']
bpy.ops.mesh.primitive_cube_add(location=(0, 0, 2))
bpy.data.scenes.remove(bpy.data.scenes['C'])
bpy.context.scene.name
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.scene.name, None))
bpy.context.window.scene = bpy.data.scenes['Scene']
bpy.ops.scene.new(type='LINK_COPY')
bpy.ops.scene.delete()
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.scene.name, None))
bpy.ops.scene.delete()
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.scene.name, None))
bpy.ops.scene.delete()
bpy.data.objects['Cube.001'].users
bpy.data.objects['Scene'].location = (bpy.data.objects['Cube.001'].users, bpy.data.collections['Collection'].users, bpy.data.worlds['World'].users)
