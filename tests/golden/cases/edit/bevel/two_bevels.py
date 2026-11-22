# WP 2.11: two bevels in one Edit Mode session: the second reuses the element slots the
# first one freed, in the iteration order of Blender's `Set<BMFace *>` of rebuilt faces (frap
# models their addresses in the mempool chunk, WP 8.2 `bevel/ptrset.rs`): index-exact.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.bevel(offset=0.2, segments=2)
bpy.ops.frap.select_elements(type='EDGE', indices=[3, 4])
bpy.ops.mesh.bevel(offset=0.1, segments=3)
