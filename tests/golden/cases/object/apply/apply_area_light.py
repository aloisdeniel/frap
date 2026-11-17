# WP 1.21 (T0), object.transform_apply on an area light: only the scale is accepted (into size
# and size_y, the square becomes a rectangle).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.select_all(action='DESELECT')
bpy.data.lights['Light'].type = 'AREA'
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Light'].scale = (2, 3, 1)
bpy.ops.object.transform_apply()
bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
bpy.data.lights['Light'].size
bpy.data.lights['Light'].size_y
bpy.data.lights['Light'].shape
