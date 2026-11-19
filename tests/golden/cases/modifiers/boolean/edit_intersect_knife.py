# WP 4.6: mesh.intersect (Intersect (Knife), EXACT; T3). A plane added in Edit Mode cuts the
# default cube's four side faces (Merge: the cut line is shared); with the default CUT the
# plane is detached from the cube along the cut. Blender 5.2.4 ends with V28 E44 F20 (frap
# too; the bands allow a different layout, T3). The selection right after the operator is
# checked by the unit tests in crates/frap-ops-mesh/src/ops/intersect.rs.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T3 check=verts:26-30,edges:42-46,faces:19-21
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.primitive_plane_add(size=3, location=(0, 0, 0.25))
bpy.ops.mesh.intersect(separate_mode='NONE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.primitive_plane_add(size=3, location=(0, 0, -0.5), rotation=(0.2, 0, 0))
bpy.ops.mesh.intersect()
bpy.ops.object.mode_set(mode='OBJECT')
