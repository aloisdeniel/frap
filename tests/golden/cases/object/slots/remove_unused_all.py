# WP 1.20 (T0), object.material_slot_remove_unused / material_slot_remove_all work on the
# selected mesh objects (an unselected active object is left alone): unused slots are removed
# (CANCELLED when every slot is used), remove_all empties every selected mesh, objects sharing a
# mesh follow; the polls need an active mesh with slots ('Context missing active object'
# without one, 'failed, context is incorrect' for a camera or a mesh without slots).
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Plane tier=T0
import bpy
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.data.meshes['Plane'].materials.append(None)
bpy.data.meshes['Plane'].materials.append(None)
bpy.data.meshes['Cube'].materials.append(None)
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Plane'].select_set(False)
bpy.ops.object.material_slot_remove_unused()
bpy.ops.object.material_slot_remove_unused()
bpy.data.objects.new('Twin', bpy.data.meshes['Cube'])
bpy.data.collections['Collection'].objects.link(bpy.data.objects['Twin'])
bpy.data.objects['Plane'].select_set(True)
bpy.data.meshes['Plane'].polygons[0].material_index = 5
bpy.ops.object.material_slot_remove_unused()
bpy.ops.object.material_slot_remove_all()
bpy.ops.object.material_slot_remove_all()
bpy.ops.object.material_slot_remove_unused()
bpy.ops.object.material_slot_remove()
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.object.material_slot_add()
bpy.ops.object.material_slot_remove_all.poll()
bpy.context.view_layer.objects.active = None
bpy.ops.object.material_slot_add()
bpy.ops.object.material_slot_remove()
bpy.ops.object.material_slot_remove_unused()
bpy.ops.object.material_slot_remove_all()
