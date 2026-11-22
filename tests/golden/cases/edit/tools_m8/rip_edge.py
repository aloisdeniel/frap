# WP 8.3.3: mesh.rip_edge / mesh.rip_edge_move with an explicit ray (location, direction):
# a cube corner extended along the edge closest to the ray, the default zero direction
# (towards location), a selected edge whose vertices follow it ("tricky extend"), a vertex
# with two selected edges left alone, an object transform, and the macro with a translation.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.mesh.rip_edge(location=(3.0, 3.0, 3.0), direction=(-1.0, -0.2, -0.5))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[7])
bpy.ops.mesh.rip_edge(location=(0.0, 3.0, 0.0))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[3])
bpy.ops.mesh.rip_edge(location=(0.0, 0.0, 5.0), direction=(0.0, 0.0, -1.0))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[5, 6])
bpy.ops.mesh.rip_edge(location=(5.0, 0.0, 0.0), direction=(-1.0, 0.0, 0.0))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[2, 4])
bpy.ops.mesh.rip_edge_move(MESH_OT_rip_edge={'location': (-4.0, 1.0, 0.0), 'direction': (1.0, 0.0, 0.0)}, TRANSFORM_OT_translate={'value': (0.0, 0.0, 0.5)})
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=2, location=(4, 0, 0), rotation=(0.0, 0.0, 0.7))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[6, 9, 12])
bpy.ops.mesh.rip_edge_move(MESH_OT_rip_edge={'location': (4.0, 3.0, 2.0), 'direction': (0.0, -1.0, -0.6)}, TRANSFORM_OT_translate={'value': (0.2, 0.3, 0.0)})
bpy.ops.mesh.rip_edge()
