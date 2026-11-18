# WP 2.7: mesh.dissolve_edges between two triangles keeps both corners with
# use_preserve_quads (the default) and dissolves them without it (the circle's fan).
# golden: startup=factory compare=objects,mesh:Circle,mesh:Circle.001 tier=T1
import bpy
bpy.ops.mesh.primitive_circle_add(vertices=6, fill_type='TRIFAN')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 6])
bpy.ops.mesh.dissolve_edges()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_circle_add(vertices=6, fill_type='TRIFAN', location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 6])
bpy.ops.mesh.dissolve_edges(use_preserve_quads=False)
bpy.ops.object.mode_set(mode='OBJECT')
