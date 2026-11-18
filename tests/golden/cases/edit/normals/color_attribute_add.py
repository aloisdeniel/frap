# WP 2.9: geometry.color_attribute_add defaults: name '' -> "Attribute", FLOAT_COLOR on
# POINT, filled with opaque black, made the active and default colour; a byte colour on
# CORNER is sRGB-encoded; the second colour does
# not replace the default colour; in Edit Mode the attribute is a BMesh layer.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Plane tier=T1
import bpy
bpy.ops.geometry.color_attribute_add()
bpy.data.objects['Cube'].name = bpy.data.meshes['Cube'].color_attributes.active_color_name
bpy.ops.geometry.color_attribute_add(name='Paint', domain='CORNER', data_type='BYTE_COLOR', color=(0.5, 0.2, 1.0, 0.25))
bpy.data.objects['Attribute'].name = bpy.data.meshes['Cube'].attributes.default_color_name
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cube'].attributes.render_color_index
bpy.context.scene.cursor.location.y = bpy.data.meshes['Cube'].attributes.active_color_index
bpy.context.scene.cursor.location.z = bpy.data.meshes['Cube'].attributes.find('Paint')
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.ops.geometry.color_attribute_add(name='Col', color=(1.0, 0.5, 0.25, 1.0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.geometry.color_attribute_add(name='Col', domain='CORNER', color=(0.1, 0.2, 0.3, 0.4))
bpy.ops.geometry.color_attribute_add(name='Bytes', data_type='BYTE_COLOR', color=(0.25, 0.75, 0.0, 1.0))
