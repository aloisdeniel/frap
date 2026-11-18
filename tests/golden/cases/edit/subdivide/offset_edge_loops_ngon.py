# WP 2.6: mesh.offset_edge_loops beside n-gons: three rim edges of a filled circle (the n-gon
# is cut by walking back along the chain, BM_face_split_n with its temporary face copy), the
# same with use_cap_endpoint, and a ring on a cube whose top was subdivided (5-gon sides).
# golden: startup=factory compare=objects,mesh:Circle,mesh:Circle.001,mesh:Cube.001 tier=T1
import bpy
bpy.ops.mesh.primitive_circle_add(vertices=10, fill_type='NGON')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[2, 3, 4])
bpy.ops.mesh.offset_edge_loops()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_circle_add(vertices=10, fill_type='NGON', location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[2, 3, 4])
bpy.ops.mesh.offset_edge_loops(use_cap_endpoint=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(location=(-3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.subdivide()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 1, 3, 4])
bpy.ops.mesh.offset_edge_loops()
bpy.ops.mesh.offset_edge_loops(use_cap_endpoint=True)
