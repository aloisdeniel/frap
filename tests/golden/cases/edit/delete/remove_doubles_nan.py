# M2 stabilization round 3: NaN coordinates never merge: BLI_kdtree's `while (co[++i] < pivot)`
# stops on NaN and the range search finds no neighbour, so mesh.remove_doubles reports
# "Removed 0 vertices" on an all-NaN cube and Auto Merge after a transform welds nothing (frap
# used to run off the end of the kd-tree and answer INTERNAL). Mixed NaN/finite points still
# merge their finite doubles. The positions are reset before the dump (GoldenState positions
# are finite floats), so the mesh comparison pins the topology.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T1
import bpy
bpy.data.meshes['Cube'].vertices.foreach_set('co', [float('nan')] * 24)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.remove_doubles()
bpy.context.scene.tool_settings.use_mesh_automerge = True
bpy.ops.transform.translate(value=(1, 0, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].vertices.foreach_set('co', [1, 1, 1, 1, 1, -1, 1, -1, 1, 1, -1, -1, -1, 1, 1, -1, 1, -1, -1, -1, 1, -1, -1, -1])
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.data.meshes['Cube.001'].vertices.foreach_set('co', [float('nan'), 0, 0, 0, float('nan'), 0, float('nan'), float('nan'), float('nan'), 1, 1, float('nan'), 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2])
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.remove_doubles()
bpy.data.meshes['Cube.001'].total_vert_sel
bpy.ops.transform.translate(value=(0, 0, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube.001'].vertices.foreach_set('co', [1, 1, 1, 1, 1, -1, 1, -1, 1, 1, -1, -1, -1, 1, 1, -1, 1, -1])
