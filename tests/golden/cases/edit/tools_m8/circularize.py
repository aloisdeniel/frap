# WP 8.3.3: mesh.circularize (new in 5.x, "To Circle"): a face region of a jittered grid
# (its boundary loop) with the least-squares fit, CONTRACT, a custom radius and rotation,
# uneven spacing (regular off), projection back on the mesh (flatten < 1), locked axes, an
# open edge chain, and a chain ending on the X mirror plane of a MIRROR modifier; fewer than
# three selected vertices: CANCELLED.
# golden: startup=factory compare=objects,context,mesh:Grid,mesh:Plane tier=T1
import bpy
bpy.ops.object.delete()
bpy.ops.mesh.primitive_grid_add(x_subdivisions=8, y_subdivisions=8, size=4)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.transform.vertex_random(offset=0.15, seed=31)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[9, 10, 11, 17, 18, 19, 25, 26, 27])
bpy.ops.mesh.circularize()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[36, 37, 38, 44, 45, 46])
bpy.ops.mesh.circularize(fit_method='CONTRACT', angle=0.3, regular=False, factor=0.8)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[33, 34, 41, 42])
bpy.ops.mesh.circularize(use_custom_radius=True, custom_radius=0.6, flatten=0.0)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[4, 5, 12, 13, 20, 21])
bpy.ops.mesh.circularize(flatten=0.4, lock=(False, False, True))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[100, 103, 106, 109, 112])
bpy.ops.mesh.circularize()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1])
bpy.ops.mesh.circularize()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_plane_add(size=2, location=(5, 0, 0))
bpy.ops.object.modifier_add(type='MIRROR')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.subdivide(number_cuts=3)
bpy.ops.transform.translate(value=(1.0, 0.0, 0.0))
bpy.ops.mesh.circularize(flatten=0.0)
bpy.context.object.modifiers['Mirror'].use_axis[1] = True
bpy.ops.transform.translate(value=(0.0, 1.0, 0.3))
bpy.ops.mesh.circularize(factor=0.5)
