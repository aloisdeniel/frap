# WP 2.6: mesh.offset_edge_loops doubles the selected loops (new loops on the old vertices,
# only they stay selected): a closed loop around a cylinder, an open loop on a grid (end caps
# joined away, then kept with use_cap_endpoint), and nothing selected (CANCELLED).
# golden: startup=factory compare=objects,context,mesh:Cylinder,mesh:Grid,mesh:Grid.001 tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8, depth=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.loopcut(edge_index=1, object_index=0)
bpy.ops.mesh.offset_edge_loops()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.offset_edge_loops()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[5, 9, 13])
bpy.ops.mesh.offset_edge_loops()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, location=(6, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='EDGE', indices=[5, 9, 13])
bpy.ops.mesh.offset_edge_loops(use_cap_endpoint=True)
bpy.context.scene.tool_settings.mesh_select_mode[:]
