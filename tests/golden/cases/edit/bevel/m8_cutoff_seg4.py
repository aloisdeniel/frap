# WP 8.2: vmesh_method CUTOFF with four segments on cube corners and with two
# segments on an ico sphere (valence 5).
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Icosphere tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.2, segments=4, vmesh_method='CUTOFF')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.1, segments=2, vmesh_method='CUTOFF')
