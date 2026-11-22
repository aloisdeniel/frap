# WP 8.2: affect='VERTICES' with three segments on valence-5 ico sphere vertices
# (vertex-only ADJ vertex meshes, odd segments).
# golden: startup=factory compare=objects,context,mesh:Icosphere tier=T1
import bpy
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 5])
bpy.ops.mesh.bevel(offset=0.2, segments=3, affect='VERTICES')
