# WP 3.9: round trip (§17.2) of the mug of modifier_stack.py exported with export_apply=True:
# the evaluated meshes come back as plain meshes. The importer stores the normals as custom
# normals (INT16_2D, each in its corner fan's space) and the re-export decodes them; on the
# flat-shaded subdivided body the float rounding of frap's fan spaces puts some corners one
# quantisation step away from Blender's (at most 1.0e-4 per component), so this case compares
# at tol=2e-4 (D-119). The first exports compare at the default tolerance in modifier_stack.py.
# golden: startup=factory tier=T2 tol=2e-4 compare=glb:out/mug_rt.glb glb_structure
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
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/mug_applied.glb')
bpy.ops.export_scene.gltf(filepath='out/mug_rt.glb')
