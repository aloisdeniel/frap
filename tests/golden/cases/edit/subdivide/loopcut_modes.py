# WP 2.6: mesh.loopcut in face select mode switches every edit mesh to edge select mode (and
# tool_settings.mesh_select_mode follows); the ring through a cylinder's side quads is closed.
# golden: startup=factory compare=objects,context,mesh:Cylinder,mesh:Cube tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(3, 0, 0))
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.loopcut(number_cuts=1, edge_index=1, object_index=0)
bpy.context.scene.tool_settings.mesh_select_mode[:]
bpy.ops.mesh.loopcut(number_cuts=3, edge_index=2, object_index=0)
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.mesh.loopcut(number_cuts=2, edge_index=5, object_index=1)
