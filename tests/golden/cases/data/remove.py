# WP 1.22: `bpy.data.<coll>.remove(id, do_unlink=True)`: references are cleared (material slots
# become empty, the scene loses its camera, children lose their parent), and objects using the
# removed data are removed with it; `do_unlink=False` refuses an ID that still has users. The
# view layer is updated before the Cube is linked again (Blender syncs bases lazily).
# golden: startup=factory tier=T0 compare=objects,materials,context
import bpy
bpy.data.objects['Cube'].parent = bpy.data.objects['Light']
bpy.data.objects['Light'].location = (0, 5, 0)
bpy.data.objects.remove(bpy.data.objects['Light'], do_unlink=False)
bpy.data.lights.remove(bpy.data.lights['Light'], do_unlink=False)
bpy.data.cameras.remove(bpy.data.cameras['Camera'], do_unlink=False)
bpy.data.meshes.remove(bpy.data.meshes['Cube'], do_unlink=False)
bpy.data.materials.remove(bpy.data.materials['Material'], do_unlink=False)
bpy.data.collections.remove(bpy.data.collections['Collection'], do_unlink=False)
bpy.data.objects.remove(bpy.data.objects['Light'])
bpy.data.objects['Cube'].location = (1, 0, 0)
bpy.data.objects.new('Twin', bpy.data.meshes['Cube'])
bpy.data.materials.remove(bpy.data.materials['Material'])
bpy.data.cameras.remove(bpy.data.cameras['Camera'])
bpy.data.objects.new('Unused', None)
bpy.data.objects.remove(bpy.data.objects['Unused'], do_unlink=False)
bpy.data.objects.remove(None)
bpy.data.objects.remove(bpy.data.meshes['Cube'])
bpy.data.meshes.remove(bpy.data.objects['Cube'])
bpy.data.worlds.remove(bpy.data.worlds['World'])
bpy.data.materials.new('Spare')
bpy.data.materials.remove(bpy.data.materials['Spare'], do_unlink=False)
bpy.data.images.new('Img', 4, 4)
bpy.data.images.remove(bpy.data.images['Img'])
bpy.data.collections.new('Extra')
bpy.data.collections['Collection'].children.link(bpy.data.collections['Extra'])
bpy.data.objects['Cube'].select_set(False)
bpy.data.collections.remove(bpy.data.collections['Collection'])
bpy.context.view_layer.update()
bpy.data.collections.remove(bpy.data.collections['Extra'], do_unlink=False)
bpy.data.objects.new('Holder', bpy.data.meshes.new('Held'))
bpy.data.meshes.remove(bpy.data.meshes['Held'])
bpy.data.objects['Holder'].name
bpy.context.scene.collection.objects.link(bpy.data.objects['Cube'])
