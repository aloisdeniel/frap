# WP 2.4: edit/select/non_manifold.py cut after its line 13,
#   bpy.ops.mesh.select_non_manifold(extend=False, use_wire=False, use_multi_face=False)
# after selecting everything, so that call's effect (extend=False: the border only)
# reaches the dump.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=3)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_non_manifold()
bpy.ops.frap.select_elements(type='VERT', indices=[7])
bpy.ops.mesh.select_non_manifold(extend=False, use_boundary=False)
bpy.ops.mesh.select_non_manifold(extend=False, use_verts=False)
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.select_non_manifold(extend=False, use_wire=False, use_multi_face=False)
