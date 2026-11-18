# WP 2.11: two bevels in one Edit Mode session: the second reuses the element slots the
# first one freed. Blender frees the replaced faces in the hash order of their pointers (not
# reproducible), frap in rebuild order, so the face order is compared canonically.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.bevel(offset=0.2, segments=2)
bpy.ops.frap.select_elements(type='EDGE', indices=[3, 4])
bpy.ops.mesh.bevel(offset=0.1, segments=3)
