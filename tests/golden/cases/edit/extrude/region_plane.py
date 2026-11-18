# WP 2.5: mesh.extrude_region on a lone plane (no neighbour): the original is kept and flipped,
# the copy becomes the cap; then again with use_normal_flip.
# golden: startup=empty compare=objects,context,mesh:Plane,mesh:Plane.001 tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_plane_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.extrude_region()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.extrude_region(use_normal_flip=True)
