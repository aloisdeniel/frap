# WP 8.3.3: mesh.reorder_vertices_spatial: the default cube with three materials (the leaf
# is split by material index), a 60x60 grid stretched along X (3600 faces: split along the
# dominant axis of the face centres, then by material) with its UV map and a vertex group,
# and the call in Edit Mode (the mesh is rewritten from the edit BMesh when Edit Mode is
# left); without an active mesh the poll fails.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T1
import bpy
bpy.ops.object.material_slot_add()
bpy.ops.object.material_slot_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[1, 4])
bpy.context.object.active_material_index = 1
bpy.ops.object.material_slot_assign()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[2])
bpy.ops.mesh.reorder_vertices_spatial()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.reorder_vertices_spatial()
bpy.ops.mesh.primitive_grid_add(x_subdivisions=60, y_subdivisions=60, size=4, location=(5, 0, 0), rotation=(0.3, 0.2, 0.1))
bpy.ops.object.vertex_group_add()
bpy.ops.object.material_slot_add()
bpy.ops.object.material_slot_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.transform.resize(value=(1.5, 1.0, 1.0))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2, 61, 62, 63, 100, 1000, 2000, 3720])
bpy.ops.object.vertex_group_assign()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[5, 6, 7, 8, 1500, 1501, 3599])
bpy.context.object.active_material_index = 1
bpy.ops.object.material_slot_assign()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.reorder_vertices_spatial()
bpy.ops.object.select_all(action='DESELECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
bpy.ops.mesh.reorder_vertices_spatial()
