# WP 8.1: uv.unwrap(method='ANGLE_BASED') on the factory cube cut into the cross layout: ABF++
# keeps the flat faces' angles (equal to LSCM there); a bare uv.unwrap() takes ANGLE_BASED from
# the factory tool settings (D-108 closed: no UV_ANGLE_BASED_AS_CONFORMAL hint any more), and
# without seams the closed cube fails with Blender's WARNING.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=1e-5
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.unwrap()
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 2, 3, 5, 6, 9, 11])
bpy.ops.mesh.mark_seam()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.unwrap(method='ANGLE_BASED')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cube'].uv_layers.active.data[5].uv[0]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.unwrap(margin=0.05, fill_holes=False)
bpy.ops.object.mode_set(mode='OBJECT')
