# WP 2.9: the active attribute is an index (Mesh.attributes_active_index). Renaming the active
# attribute through RNA keeps it active (attribute_set / attribute_remove still run); removing
# an earlier attribute through RNA (attributes.remove, uv_layers.remove) leaves the index, so
# the next attribute becomes active, or none past the end. 'id' is not a built-in mesh
# attribute: any domain and type can be added under that name.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T1
import bpy
bpy.ops.geometry.attribute_add(name='foo', domain='FACE', data_type='FLOAT')
bpy.data.meshes['Cube'].attributes['foo'].name = 'bar'
bpy.data.objects['Camera'].name = bpy.data.meshes['Cube'].attributes.active.name
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cube'].attributes.active_index
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.attribute_set(value_float=0.5)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.geometry.attribute_add(name='baz', domain='POINT', data_type='FLOAT')
bpy.ops.geometry.attribute_add(name='qux', domain='EDGE', data_type='INT')
bpy.data.meshes['Cube'].attributes.active = bpy.data.meshes['Cube'].attributes['baz']
bpy.data.meshes['Cube'].attributes['baz'].name = 'baz2'
bpy.ops.geometry.attribute_remove()
bpy.data.objects['Light'].name = bpy.data.meshes['Cube'].attributes.active.name
bpy.context.scene.cursor.location.y = bpy.data.meshes['Cube'].attributes.active_index
bpy.data.meshes['Cube'].attributes.active = bpy.data.meshes['Cube'].attributes['bar']
bpy.data.meshes['Cube'].attributes.remove(bpy.data.meshes['Cube'].attributes['sharp_face'])
bpy.context.scene.cursor.location.z = bpy.data.meshes['Cube'].attributes.active_index
bpy.data.meshes['Cube'].attributes.active = bpy.data.meshes['Cube'].attributes['position']
bpy.data.meshes['Cube'].uv_layers.remove(bpy.data.meshes['Cube'].uv_layers['UVMap'])
bpy.context.scene.cursor.rotation_euler.y = bpy.data.meshes['Cube'].attributes.active_index
bpy.data.meshes['Cube'].attributes.active = None
bpy.ops.geometry.attribute_add(name='id', domain='FACE', data_type='FLOAT')
bpy.ops.mesh.primitive_cube_add(size=1, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.geometry.attribute_add(name='id', domain='POINT', data_type='FLOAT_VECTOR')
bpy.ops.object.mode_set(mode='OBJECT')
