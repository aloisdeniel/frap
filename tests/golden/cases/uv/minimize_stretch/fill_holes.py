# WP 8.1: uv.minimize_stretch on a grid with a hole: with fill_holes=False the hole's border is
# a second boundary (pinned); with fill_holes=True (the default) it is filled virtually, its
# vertices move, and the filled faces' stretch counts one fifth.
# golden: startup=empty compare=objects,context,mesh:Grid,mesh:Grid.001 tier=T2 tol=1e-5
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=8, y_subdivisions=8, size=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[18, 19, 26, 27])
bpy.ops.mesh.delete(type='FACE')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.resize(value=(1.0, 0.5, 1.0))
bpy.ops.uv.unwrap(method='CONFORMAL', fill_holes=False)
bpy.ops.uv.minimize_stretch(fill_holes=False, iterations=8)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=8, y_subdivisions=8, size=2, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[18, 19, 26, 27])
bpy.ops.mesh.delete(type='FACE')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.resize(value=(1.0, 0.5, 1.0))
bpy.ops.uv.unwrap(method='CONFORMAL', fill_holes=False)
bpy.ops.uv.minimize_stretch(iterations=8)
bpy.ops.object.mode_set(mode='OBJECT')
