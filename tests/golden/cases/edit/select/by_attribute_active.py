# M2 W2 integration: mesh.select_by_attribute with an active attribute (Mesh.attributes_active,
# WP 2.9). A BOOLEAN point attribute selects its true vertices; a BOOLEAN face attribute selects
# faces; a FLOAT active attribute and a corner-domain one fail the poll with Blender's messages.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.data.meshes['Cube'].attributes.new('flag', 'BOOLEAN', 'POINT')
bpy.data.meshes['Cube'].attributes['flag'].data[0].value = True
bpy.data.meshes['Cube'].attributes['flag'].data[3].value = True
bpy.data.meshes['Cube'].attributes['flag'].data[5].value = True
bpy.data.meshes['Cube'].attributes.active = bpy.data.meshes['Cube'].attributes['flag']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_by_attribute()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes.new('fflag', 'BOOLEAN', 'FACE')
bpy.data.meshes['Cube'].attributes['fflag'].data[2].value = True
bpy.data.meshes['Cube'].attributes.active = bpy.data.meshes['Cube'].attributes['fflag']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_by_attribute()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes.new('weight', 'FLOAT', 'POINT')
bpy.data.meshes['Cube'].attributes.active = bpy.data.meshes['Cube'].attributes['weight']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_by_attribute()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes.new('cflag', 'BOOLEAN', 'CORNER')
bpy.data.meshes['Cube'].attributes.active = bpy.data.meshes['Cube'].attributes['cflag']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_by_attribute()
