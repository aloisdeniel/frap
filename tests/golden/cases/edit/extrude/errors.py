# WP 2.5: argument errors of the extrude and inset operators (T0) and their poll outside Edit Mode.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.mesh.extrude_region()
bpy.ops.mesh.inset(thickness=0.1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.extrude_region(use_normal_fl=True)
bpy.ops.mesh.inset(thickness='a')
bpy.ops.mesh.extrude_repeat(offset=(1, 2))
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={'valu': (0, 0, 1)})
bpy.ops.mesh.extrude_region_move(MESH_OT_extrude_region={'use_normal_flip': 'x'})
bpy.ops.mesh.inset(thickness=-1, use_individual=True)
