# WP 4.6: two mesh.extrude_manifold in a row: a side face pulled out diagonally, then the top
# face pushed down into the side walls, which are cut where the pushed vertices reach them.
# T2: the first extrude's use_dissolve_ortho_edges frees vertices in another order than
# Blender (WP 2.5's extrude_region), so the second step's new vertices reuse other slots; the
# geometry is Blender's.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=1e-5
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.mesh.extrude_manifold(TRANSFORM_OT_translate={"value": (0.3, 0.4, 0)})
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.extrude_manifold(TRANSFORM_OT_translate={"value": (0, 0, -0.75)})
bpy.data.meshes['Cube'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
