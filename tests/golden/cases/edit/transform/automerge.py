# M2 stabilization: Auto Merge Vertices (`tool_settings.use_mesh_automerge`): after an
# edit-mode transform the selected vertices weld into unselected ones (and into each other)
# within `double_threshold` (`special_aftertrans_update__mesh` -> `EDBM_automerge`); in face
# select mode the faces are re-selected from their edges. Vertex 1 is moved onto vertex 0;
# a vertex moved to within the threshold also merges, one moved farther does not; an extrude
# moved back onto its base welds the extruded vertices into the base ones.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.context.scene.tool_settings.use_mesh_automerge = True
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[1])
bpy.ops.transform.translate(value=(0, 0, 2))
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[2])
bpy.ops.transform.translate(value=(0, 1.9995, 0))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[3])
bpy.ops.transform.translate(value=(0, 0, 0.5))
bpy.context.scene.tool_settings.double_threshold = 0.6
bpy.ops.transform.translate(value=(0, 0, 0))
bpy.context.scene.tool_settings.double_threshold = 0.001
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, 0)})
bpy.data.meshes['Cube'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
