# WP 2.13 workflow (cleanup): a table top subdivided into a dense grid, a face deleted and the
# hole filled again, coplanar faces merged back with mesh.dissolve_limited, then a duplicated
# copy of the whole top merged back onto itself with mesh.remove_doubles.
# golden: startup=empty compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(size=2, scale=(1, 0.6, 0.05))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=3)
bpy.data.meshes['Cube'].total_face_sel
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[20])
bpy.ops.mesh.delete(type='FACE')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.fill_holes(sides=4)
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.dissolve_limited()
bpy.data.meshes['Cube'].total_face_sel
bpy.ops.mesh.duplicate()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.remove_doubles(threshold=0.0001)
bpy.data.meshes['Cube'].total_vert_sel
