# WP 2.13 workflow (chair cushion): a flattened cube subdivided twice with smoothness (the
# second pass on the whole result), relaxed with mesh.vertices_smooth, then shaded smooth.
# golden: startup=empty compare=objects,context,mesh:Cube tier=T1 tol=1e-4
import bpy
bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 0, 0.5), scale=(1, 1, 0.25))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=1, smoothness=1)
bpy.ops.mesh.subdivide(number_cuts=1)
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.mesh.vertices_smooth(factor=0.5, repeat=3)
bpy.ops.mesh.faces_shade_smooth()
