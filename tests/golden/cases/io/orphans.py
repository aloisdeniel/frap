# WP 1.23: data-blocks without users are not written: a deleted object's mesh and an unused
# material are gone after the save and open, a fake-user material stays.
# Paths are in the work directory: Blender's save does not create directories.
# golden: startup=factory tier=T1 compare=objects,materials,context
import bpy
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.delete()
bpy.data.materials.new('Loose')
bpy.data.materials.new('Kept')
bpy.data.materials['Kept'].use_fake_user = True
bpy.ops.wm.save_as_mainfile(filepath='o.frap')
bpy.ops.wm.open_mainfile(filepath='o.frap')
