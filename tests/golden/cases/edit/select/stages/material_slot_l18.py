# WP 2.4: edit/select/material_slot.py cut after its line 18,
#   bpy.ops.object.material_slot_select()
# so that call's effect reaches the dump (later lines of the full case replace it).
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Plane tier=T1
import bpy
bpy.data.materials.new('Red')
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['Red'])
bpy.data.meshes['Cube'].polygons[2].material_index = 1
bpy.data.meshes['Cube'].polygons[4].material_index = 1
bpy.ops.object.material_slot_select()
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.data.meshes['Plane'].materials.append(bpy.data.materials['Red'])
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.context.object.active_material_index = 1
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.object.material_slot_select()
