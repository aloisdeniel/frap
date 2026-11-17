# WP 1.20 (T0), object.duplicate with the factory "Duplicate Data" preferences: the copy
# 'Cube.001' gets its own mesh 'Cube.001' (same geometry) but shares the material; the copy is
# selected and active, the original deselected; nothing selected → CANCELLED; linked=True
# shares the data (no 'Light.001'/'Camera.001' data), and the copy of the active object becomes
# active.
# golden: startup=factory compare=objects,context,materials,mesh:Cube.001 tier=T0
import bpy
bpy.ops.object.duplicate()
bpy.data.meshes['Cube.001']
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.duplicate()
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.object.duplicate(linked=True)
bpy.data.lights['Light.001']
bpy.data.cameras['Camera.001']
bpy.ops.object.duplicate(mode='ROTATION')
bpy.data.lights['Light.001']
bpy.data.cameras['Camera.001']
bpy.ops.object.duplicate.poll()
