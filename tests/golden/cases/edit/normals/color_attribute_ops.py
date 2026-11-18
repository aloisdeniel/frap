# WP 2.9: geometry.color_attribute_remove (the active colour; active/default colour move to
# the clamped index), color_attribute_render_set (default colour; a name that is not a colour
# attribute is cancelled), color_attribute_duplicate and color_attribute_convert (Object
# Mode only; POINT <-> CORNER, FLOAT_COLOR <-> BYTE_COLOR; the attribute moves to the end).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.geometry.color_attribute_add(name='A', color=(0.2, 0.4, 0.6, 1.0))
bpy.ops.geometry.color_attribute_add(name='B', domain='CORNER', color=(0.9, 0.1, 0.3, 0.5))
bpy.ops.geometry.color_attribute_add(name='C', data_type='BYTE_COLOR', color=(0.5, 0.5, 0.5, 1.0))
bpy.ops.geometry.color_attribute_render_set(name='B')
bpy.ops.geometry.color_attribute_render_set(name='UVMap')
bpy.ops.geometry.color_attribute_render_set()
bpy.ops.geometry.color_attribute_duplicate()
bpy.data.meshes['Cube'].attributes.active_color = bpy.data.meshes['Cube'].attributes['B']
bpy.ops.geometry.color_attribute_convert(domain='POINT', data_type='BYTE_COLOR')
bpy.data.meshes['Cube'].attributes.active_color = bpy.data.meshes['Cube'].attributes['A']
bpy.ops.geometry.color_attribute_convert(domain='CORNER')
bpy.data.objects['Cube'].name = bpy.data.meshes['Cube'].attributes.default_color_name
bpy.data.meshes['Cube'].attributes.active_color = bpy.data.meshes['Cube'].attributes['B']
bpy.ops.geometry.color_attribute_remove()
bpy.data.objects['B'].name = bpy.data.meshes['Cube'].attributes.default_color_name
bpy.data.objects['A'].name = 'X' + bpy.data.meshes['Cube'].color_attributes.active_color_name
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.geometry.color_attribute_duplicate()
bpy.ops.geometry.color_attribute_convert()
bpy.ops.geometry.color_attribute_render_set(name='A')
bpy.ops.geometry.color_attribute_remove()
bpy.ops.geometry.color_attribute_remove()
bpy.ops.geometry.color_attribute_remove()
bpy.ops.geometry.color_attribute_remove()
