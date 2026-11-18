# WP 4.6: mesh.extrude_manifold on a grid. A zero extrude is folded back into the grid; the
# face pushed sideways into its neighbour cuts it, and the region's side faces flip with the
# push. T2: extrude_region with use_dissolve_ortho_edges orders the second extrude's edges and
# faces differently from Blender (WP 2.5), the geometry is Blender's.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T2 tol=1e-5
import bpy
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete()
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[4])
bpy.ops.mesh.extrude_manifold(TRANSFORM_OT_translate={"value": (0, 0, 0)})
bpy.data.meshes['Grid'].total_face_sel
bpy.ops.mesh.extrude_manifold(TRANSFORM_OT_translate={"value": (0.25, 0, 0)})
bpy.data.meshes['Grid'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
