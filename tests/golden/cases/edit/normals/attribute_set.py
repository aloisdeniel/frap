# WP 2.9: mesh.attribute_set writes the active attribute on the selected elements of its
# domain (CORNER: corners of selected faces in face mode, of selected vertices in vertex
# mode), with the value of the property for its type (byte colours sRGB-encoded); no active
# attribute fails the poll.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.attribute_set(value_float=1.0)
bpy.ops.geometry.attribute_add(name='f', domain='POINT', data_type='FLOAT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 3, 5])
bpy.ops.mesh.attribute_set(value_float=2.5)
bpy.ops.geometry.attribute_add(name='e', domain='EDGE', data_type='INT')
bpy.ops.mesh.attribute_set(value_int=7)
bpy.ops.geometry.attribute_add(name='c', domain='CORNER', data_type='BYTE_COLOR')
bpy.ops.mesh.attribute_set(value_color=(0.5, 0.25, 1.0, 1.0))
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.geometry.attribute_add(name='v', domain='CORNER', data_type='FLOAT_VECTOR')
bpy.ops.mesh.attribute_set(value_float_vector_3d=(1.0, -2.0, 0.5))
bpy.ops.geometry.attribute_add(name='b', domain='FACE', data_type='BOOLEAN')
bpy.ops.mesh.attribute_set(value_bool=True)
bpy.ops.geometry.attribute_add(name='s', domain='POINT', data_type='INT16_2D')
bpy.ops.mesh.attribute_set(value_int_vector_2d=(3, -4))
bpy.ops.geometry.attribute_add(name='q', domain='POINT', data_type='QUATERNION')
bpy.ops.mesh.attribute_set()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.geometry.attribute_add(name='g', domain='FACE', data_type='FLOAT2')
bpy.ops.mesh.attribute_set(value_float_vector_2d=(1.0, 2.0))
