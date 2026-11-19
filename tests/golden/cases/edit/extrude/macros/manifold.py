# Enabled by WP 4.6 (M2 leftover, decision log M34W0-11): the translate step's
# use_automerge_and_split=True (EDBM_automerge_and_split) is implemented; the Blender fixture
# was blessed by WP 2.5.
# WP 2.5: mesh.extrude_manifold pushing the top face inward only moves it (V8 E12 F6).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 order=canonical
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.extrude_manifold(TRANSFORM_OT_translate={"value": (0, 0, -0.5)})
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_face_sel
