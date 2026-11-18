# WP 2.4: object.material_slot_select / material_slot_deselect select and deselect the faces of
# the active slot's material in Edit Mode (no flush), on every object in Edit Mode that uses
# the material; CANCELLED without matching faces or outside Edit Mode (no poll).
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
bpy.data.meshes['Cube'].total_face_sel
bpy.data.meshes['Plane'].total_face_sel
bpy.ops.object.material_slot_deselect()
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.object.material_slot_deselect()
bpy.context.object.active_material_index = 0
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.object.material_slot_select()
bpy.ops.object.material_slot_deselect()
