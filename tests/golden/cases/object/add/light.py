# WP 1.20 (T0), object.light_add: object and data named after the type ('Point', 'Sun',
# 'Spot', 'Area'), uniquified independently; placement like the primitives (cursor, align,
# a set rotation wins); radius and scale leave the object scale at 1.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.context.scene.cursor.location = (1, 2, 3)
bpy.ops.object.light_add()
bpy.ops.object.light_add(type='SUN', radius=3, align='VIEW')
bpy.ops.object.light_add(type='SPOT', rotation=(0.5, 0, 0), align='VIEW')
bpy.ops.object.light_add(type='AREA', radius=2, location=(0, 0, 4), scale=(2, 3, 4))
bpy.data.lights['Point'].name = 'Bulb'
bpy.ops.object.light_add(type='POINT')
bpy.data.objects['Point.001'].data.name = 'Other'
bpy.data.lights['Point']
bpy.data.lights['Point.001']
bpy.ops.object.light_add(type='HEMI')
