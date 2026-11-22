# M5+M6 W0 (M3+M4 leftover, D-79 closed): `mesh.bevel(harden_normals=True)` writes the edit
# mesh's custom normals (`bevel_harden_normals`), for edges and for vertices.
# golden: startup=factory compare=mesh:Cube,mesh:Cube.001 tier=T1
import bpy
bpy.ops.object.shade_smooth()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.2, segments=2, harden_normals=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.ops.object.shade_smooth()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.3, segments=3, affect='VERTICES', harden_normals=True)
bpy.ops.object.mode_set(mode='OBJECT')
