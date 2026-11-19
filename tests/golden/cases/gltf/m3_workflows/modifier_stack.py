# WP 3.9: a modelled mug exported with its modifier stack applied (export_apply=True). Half of
# the body is mirrored (MIRROR, merge on the seam), bevelled (BEVEL, 2 segments) and subdivided
# (SUBSURF level 1); a SOLIDIFY handle torus and a TRIANGULATE coaster. Without export_apply the
# original meshes export. Round trip (§17.2) of both files: the evaluated meshes come back as
# plain meshes. The importer stores the normals as INT16_2D custom normals in each
# fan's corner space (both tools); on the flat-shaded subdivided body float rounding of the
# fan spaces puts a few corners a quantisation step apart (at most 1.0e-4 between the two
# re-exports, and Blender's own re-export moves its normals by as much), hence tol=2e-4.
# golden: startup=factory tier=T2 tol=2e-4 compare=glb:out/mug_applied.glb,glb:out/mug_raw.glb,glb:out/mug_rt.glb,glb:out/mug_raw_rt.glb
import bpy
bpy.data.objects['Cube'].name = 'Body'
bpy.data.objects['Body'].modifiers.new('Mirror', 'MIRROR')
bpy.data.objects['Body'].modifiers['Mirror'].use_axis = (True, False, False)
bpy.data.objects['Body'].modifiers['Mirror'].use_clip = True
bpy.data.objects['Body'].modifiers.new('Bevel', 'BEVEL')
bpy.data.objects['Body'].modifiers['Bevel'].segments = 2
bpy.data.objects['Body'].modifiers['Bevel'].width = 0.2
bpy.data.objects['Body'].modifiers.new('Subdivision', 'SUBSURF')
bpy.data.objects['Body'].modifiers['Subdivision'].levels = 1
bpy.ops.mesh.primitive_torus_add(major_segments=12, minor_segments=4, major_radius=0.6, minor_radius=0.1, location=(1.6, 0, 0), rotation=(1.5707963, 0, 0))
bpy.context.object.name = 'Handle'
bpy.ops.object.modifier_add(type='SOLIDIFY')
bpy.data.objects['Handle'].modifiers['Solidify'].thickness = 0.05
bpy.ops.mesh.primitive_cylinder_add(vertices=8, radius=1.5, depth=0.1, location=(0, 0, -1.1))
bpy.context.object.name = 'Coaster'
bpy.ops.object.modifier_add(type='TRIANGULATE')
bpy.ops.export_scene.gltf(filepath='out/mug_applied.glb', export_apply=True)
bpy.ops.export_scene.gltf(filepath='out/mug_raw.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/mug_applied.glb')
bpy.ops.export_scene.gltf(filepath='out/mug_rt.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/mug_raw.glb')
bpy.ops.export_scene.gltf(filepath='out/mug_raw_rt.glb')
