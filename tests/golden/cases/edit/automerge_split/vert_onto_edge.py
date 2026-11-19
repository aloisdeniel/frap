# WP 4.6: Auto Merge with split from the tool settings (`use_mesh_automerge` and
# `use_mesh_automerge_and_split`, T_AUTOSPLIT). A grid vertex moved onto another edge cuts
# that edge and welds into the cut; a vertex moved onto another vertex welds as Auto Merge
# does; a vertex moved next to an edge (farther than `double_threshold`) changes nothing.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1 tol=1e-5
import bpy
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete()
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3, size=2)
bpy.context.scene.tool_settings.use_mesh_automerge = True
bpy.context.scene.tool_settings.use_mesh_automerge_and_split = True
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[8])
bpy.ops.transform.translate(value=(-0.5, 0, 0))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.transform.translate(value=(1, 0, 0))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[2])
bpy.ops.transform.translate(value=(0, 0.6, 0.01))
bpy.data.meshes['Grid'].total_vert_sel
bpy.ops.object.mode_set(mode='OBJECT')
