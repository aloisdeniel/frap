# WP 4.6: mesh.intersect_boolean UNION and INTERSECT with use_swap (Edit Mode, EXACT; T3): a
# rotated cube added in Edit Mode is merged into the default cube; then a sphere is
# intersected with the result. Every result is one closed mesh.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T3 check=manifold,euler:2,faces:6-200
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.primitive_cube_add(size=1.5, location=(0.9, 0.2, 0.3), rotation=(0.3, 0.2, 0.5))
bpy.ops.mesh.intersect_boolean(operation='UNION', solver='EXACT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1.3)
bpy.ops.mesh.intersect_boolean(operation='INTERSECT', use_swap=True)
bpy.ops.object.mode_set(mode='OBJECT')
