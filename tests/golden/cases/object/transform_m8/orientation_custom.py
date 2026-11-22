# WP 8.3.5 (T1), custom transform orientations in Object Mode: create_orientation from the
# active object (named after it, made unique, given a name, overwritten), selected with use=True
# or select_orientation and then used by transforms without orient_type; used by name through
# orient_type; deleted (the scene orientation falls back to GLOBAL; the poll fails after that).
# golden: startup=factory compare=objects,context tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].rotation_euler = (0.3, 0.4, 0.5)
bpy.data.objects['Cube'].scale = (1, 2, 0.5)
bpy.ops.transform.create_orientation()
bpy.ops.transform.create_orientation(use=True)
bpy.ops.transform.translate(value=(1, 0, 0))
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
bpy.ops.transform.create_orientation(name='Lamp')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.transform.rotate(value=0.3, constraint_axis=(False, False, True))
bpy.ops.transform.select_orientation(orientation='Lamp')
bpy.ops.transform.resize(value=(1.5, 1, 1))
bpy.ops.transform.shear(angle=0.3)
bpy.ops.transform.select_orientation(orientation='GLOBAL')
bpy.ops.transform.translate(value=(0, 1, 0), orient_type='Cube.001')
bpy.ops.transform.rotate(value=0.2, orient_axis='X', orient_type='Lamp')
bpy.ops.transform.translate(value=(0, 0, 1), orient_type='Nope')
bpy.ops.transform.select_orientation(orientation='Nope')
bpy.ops.transform.delete_orientation()
bpy.ops.transform.select_orientation(orientation='Cube')
bpy.ops.transform.delete_orientation()
bpy.ops.transform.translate(value=(0.5, 0, 0))
bpy.ops.transform.translate(value=(0, 0.5, 0), orient_type='Lamp')
bpy.ops.transform.select_orientation(orientation='Lamp')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.translate(value=(0, 0, 0.25))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.transform.create_orientation(name='Lamp', overwrite=True)
bpy.ops.transform.translate(value=(0.5, 0, 0))
bpy.ops.transform.delete_orientation()
bpy.ops.transform.translate(value=(0.5, 0, 0))
