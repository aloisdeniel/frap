# WP 1.20 (T0), object.camera_add: object and data are both requested as 'Camera' and
# uniquified independently (renaming the factory camera data frees 'Camera' for the new data,
# so object 'Camera.001' gets data 'Camera'); align is forced to VIEW, so an unset rotation is
# the view rotation even with align='CURSOR'; a set rotation wins; scene.camera is set only
# when the scene has none; enter_editmode and scale are ignored.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.cameras['Camera'].name = 'Lens'
bpy.ops.object.camera_add()
bpy.data.objects['Camera.001'].data.name = 'NewData'
bpy.data.cameras['Camera']
bpy.data.cameras['NewData']
bpy.context.scene.camera.name
bpy.context.scene.camera = None
bpy.ops.object.camera_add(location=(1, 2, 3), rotation=(0.1, 0.2, 0.3), align='WORLD')
bpy.context.scene.camera.name
bpy.ops.object.camera_add(align='CURSOR', scale=(2, 2, 2), enter_editmode=True)
bpy.data.cameras['Camera.001']
bpy.context.mode
