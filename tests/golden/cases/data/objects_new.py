# WP 1.22: `bpy.data.objects.new(name, object_data)`: EMPTY for None, the data's type otherwise;
# unique names (lowest free `.NNN`, '' gives 'Object'); the object is in no collection until
# `collection.objects.link`; non-object data and argument errors raise Blender's errors;
# `get`/`find` see the new objects.
# golden: startup=factory tier=T0 compare=objects,context
import bpy
bpy.data.objects.new('Empty', None)
bpy.data.objects.new('Empty', None)
bpy.data.objects.new('', None)
bpy.data.objects.new('CubeTwin', bpy.data.meshes['Cube'])
bpy.data.objects.new('Cam2', bpy.data.cameras['Camera'])
bpy.data.objects.new('Lamp2', bpy.data.lights['Light'])
bpy.data.objects.new('Bad', bpy.data.materials['Material'])
bpy.data.objects.new('Bad', bpy.data.worlds['World'])
bpy.data.objects.new('Bad', bpy.data.collections['Collection'])
bpy.data.objects.new('Bad', bpy.data.scenes['Scene'])
bpy.data.objects.new('Bad', bpy.data.objects['Cube'])
bpy.data.objects.new('Bad')
bpy.data.objects.new(name='Bad')
bpy.data.objects.new()
bpy.data.objects.new(1, None)
bpy.data.objects.new('Bad', 3)
bpy.data.objects.new('Bad', None, None)
bpy.data.objects.new('Bad', object_data=None, nope=1)
bpy.data.objects.get('Empty.001').name
bpy.data.objects.get('Nope', bpy.data.objects['Cube']).name
bpy.data.objects.find('Empty.001')
bpy.data.objects['Empty.001'].users
bpy.data.collections['Collection'].objects.link(bpy.data.objects['Empty'])
bpy.data.collections['Collection'].objects.link(bpy.data.objects['Empty'])
bpy.context.scene.collection.objects.link(bpy.data.objects['CubeTwin'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Cube'])
bpy.data.collections['Collection'].objects.link(None)
bpy.data.collections['Collection'].objects.link(bpy.data.meshes['Cube'])
bpy.context.view_layer.update()
bpy.data.objects['CubeTwin'].select_set(True)
bpy.data.objects['Empty'].select_set(True)
bpy.data.objects['Empty'].location = (1, 2, 3)
bpy.data.objects['Cam2'].location = (0, 0, 0)
