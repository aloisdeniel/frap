# WP 8.11 (deviation D-124 closed): glTF export of Suzanne + shade_smooth + SUBSURF level 1
# with export_apply=True writes Blender's 2893 vertices. OpenSubdiv evaluates every corner's UV
# in its own patch, so corners sharing a vertex differ by an ulp or two and the exporter's exact
# np.unique splits them; frap's bit-exact face-varying evaluation reproduces the split. The
# re-import (one mesh vertex per glTF vertex) makes the vertex count part of the comparison.
# Only the re-imported vertices are dumped (edges and faces deleted in Edit Mode) so the
# fixture stays small; corner UVs and normals are compared in the exported file.
# golden: startup=empty compare=glb:out/suzanne.glb,mesh:Suzanne tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.shade_smooth()
bpy.data.objects['Suzanne'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.ops.export_scene.gltf(filepath='out/suzanne.glb', export_apply=True)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/suzanne.glb')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.delete(type='EDGE_FACE')
bpy.ops.object.mode_set(mode='OBJECT')
