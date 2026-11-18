# WP 4.6: mesh.intersect_boolean (Edit Mode, EXACT; T3): a smaller cube added in Edit Mode
# (selected) is cut out of the default cube (unselected). The result is one closed mesh
# (V - E + F = 2); a second call with nothing selected changes nothing.
# golden: startup=factory compare=objects,context tier=T3 check=manifold,euler:2,faces:9-30,verts:14-40
# golden: compare=mesh:Cube
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.primitive_cube_add(size=1, location=(0.5, 0.5, 0.5))
bpy.ops.mesh.intersect_boolean(operation='DIFFERENCE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.intersect_boolean(operation='UNION')
bpy.ops.object.mode_set(mode='OBJECT')
