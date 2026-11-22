# WP 8.2: every edge of an ico sphere (valence-5 corners), three segments: ADJ vertex
# meshes with five boundary vertices and a center polygon.
# golden: startup=factory compare=objects,context,mesh:Icosphere tier=T1
import bpy
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.1, segments=3)
