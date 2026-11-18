# WP 2.9: geometry.attribute_add (unique names, the new attribute becomes active, refused
# domains), attribute_remove (the activity moves one index down, to a non-internal attribute;
# required attributes are refused), attribute_convert GENERIC (domain + type, the attribute
# moves to the end and stays active) and VERTEX_GROUP; mesh.attributes.active/active_index.
# golden: startup=factory compare=objects,context,mesh:Cube.001 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(size=1)
bpy.ops.geometry.attribute_add(name='foo')
bpy.ops.geometry.attribute_add(name='foo', domain='FACE', data_type='INT')
bpy.data.objects['Cube.001'].name = bpy.data.meshes['Cube.001'].attributes.active.name
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cube.001'].attributes.active_index
bpy.ops.geometry.attribute_add(name='', domain='CORNER', data_type='FLOAT2')
bpy.ops.geometry.attribute_add(name='w', domain='CURVE')
bpy.ops.geometry.attribute_add(name='q', domain='EDGE', data_type='QUATERNION')
bpy.ops.geometry.attribute_add(name='v', domain='POINT', data_type='FLOAT_VECTOR')
bpy.data.meshes['Cube.001'].attributes['v'].data[3].vector = (1.0, 2.0, 3.0)
bpy.data.meshes['Cube.001'].attributes['foo'].data[0].value = 0.75
bpy.data.meshes['Cube.001'].attributes.active = bpy.data.meshes['Cube.001'].attributes['foo']
bpy.ops.geometry.attribute_convert(domain='FACE', data_type='FLOAT_COLOR')
bpy.context.scene.cursor.location.y = bpy.data.meshes['Cube.001'].attributes.active_index
bpy.data.meshes['Cube.001'].attributes.active = bpy.data.meshes['Cube.001'].attributes['v']
bpy.ops.geometry.attribute_convert(domain='CORNER', data_type='INT32_2D')
bpy.ops.geometry.attribute_convert(domain='EDGE', data_type='STRING')
bpy.data.meshes['Cube.001'].attributes.active = bpy.data.meshes['Cube.001'].attributes['foo.001']
bpy.ops.geometry.attribute_convert(domain='POINT', data_type='BOOLEAN')
bpy.ops.geometry.attribute_remove()
bpy.data.objects['foo.001'].name = bpy.data.meshes['Cube.001'].attributes.active.name
bpy.data.meshes['Cube.001'].attributes.active_index = 0
bpy.ops.geometry.attribute_remove()
bpy.data.meshes['Cube.001'].attributes.active = bpy.data.meshes['Cube.001'].attributes['position']
bpy.ops.geometry.attribute_remove()
bpy.data.meshes['Cube.001'].attributes.active = bpy.data.meshes['Cube.001'].attributes['foo']
bpy.ops.geometry.attribute_convert(mode='VERTEX_GROUP')
bpy.data.meshes['Cube.001'].attributes.active = None
bpy.ops.geometry.attribute_remove()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.geometry.attribute_add(name='e', domain='EDGE')
bpy.ops.geometry.attribute_add(name='sharp_face', domain='FACE', data_type='BOOLEAN')
bpy.ops.geometry.attribute_remove()
bpy.ops.geometry.attribute_convert()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Camera'].name = bpy.data.meshes['Cube.001'].attributes.active.name
