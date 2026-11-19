# WP 3.8: uv.lightmap_pack on the default cube in Edit Mode ([MESH App. B.5]): each selected
# quad gets its own rectangle (6 squares, margin 0.1%); the operator leaves Edit Mode and
# re-enters it. Then only two selected faces with a 1% margin and coarser boxes.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.lightmap_pack()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cube'].uv_layers.active.data[0].uv[0]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[1, 4])
bpy.ops.uv.lightmap_pack(PREF_MARGIN_DIV=1.0, PREF_BOX_DIV=2)
bpy.ops.object.mode_set(mode='OBJECT')
