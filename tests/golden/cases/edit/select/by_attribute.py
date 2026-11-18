# WP 2.4: mesh.select_by_attribute needs an active boolean attribute; attributes.new does not
# make the new attribute active (verified on 5.2.4), so the poll fails with Blender's message
# before and after adding one (outside Edit Mode: the generic poll message). The attribute is
# added in Object Mode: frap refuses attribute writes in Edit Mode (WP 2.2).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.mesh.select_by_attribute()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_by_attribute()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes.new('flag', 'BOOLEAN', 'POINT')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_by_attribute()
