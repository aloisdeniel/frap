# WP 8.11 (deviations D-121, D-124 closed): smooth-shaded SUBSURF meshes whose UVs come from
# face-varying Gregory end caps (SIMPLE level 3 on a 6-sided cylinder, level 4 above quality on
# a 7-gon, level 2 above quality 1 on the cube) and the cube at level 3 with SMOOTH_ALL and
# PRESERVE_CORNERS UVs, exported with export_apply=True and re-imported: the glTF vertex split
# of every mesh is Blender's.
# Only the re-imported vertices are dumped (edges and faces deleted in Edit Mode) so the
# fixture stays small; corner UVs and normals are compared in the exported file.
# golden: startup=empty compare=glb:out/caps.glb,mesh:Cylinder,mesh:Circle,mesh:Cube,mesh:Cube.001,mesh:Cube.002 tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=6)
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_circle_add(vertices=7, fill_type='NGON', location=(3, 0, 0))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_cube_add(location=(6, 0, 0))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_cube_add(location=(9, 0, 0))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_cube_add(location=(12, 0, 0))
bpy.ops.object.shade_smooth()
bpy.data.objects['Cylinder'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cylinder'].modifiers['Subdivision'].subdivision_type = 'SIMPLE'
bpy.data.objects['Cylinder'].modifiers['Subdivision'].levels = 3
bpy.data.objects['Circle'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Circle'].modifiers['Subdivision'].levels = 4
bpy.data.objects['Cube'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube'].modifiers['Subdivision'].levels = 3
bpy.data.objects['Cube'].modifiers['Subdivision'].uv_smooth = 'SMOOTH_ALL'
bpy.data.objects['Cube.001'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube.001'].modifiers['Subdivision'].levels = 3
bpy.data.objects['Cube.001'].modifiers['Subdivision'].uv_smooth = 'PRESERVE_CORNERS'
bpy.data.objects['Cube.002'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube.002'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cube.002'].modifiers['Subdivision'].quality = 1
bpy.ops.export_scene.gltf(filepath='out/caps.glb', export_apply=True)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/caps.glb')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.delete(type='EDGE_FACE')
bpy.ops.object.mode_set(mode='OBJECT')
