# WP 3.9: a lightmap atlas for two props. A cylinder and a cone get smart_project UVs
# (angle limit 66 degrees, margin 0.01); the cylinder's islands are then rescaled with
# average_islands_scale and packed again (pack_islands, AABB shape, rotation off). UV layouts
# compare within 1e-4. Round trip (§17.2).
# golden: startup=factory tier=T2 tol=1e-4 compare=glb:out/props.glb,glb:out/props_rt.glb
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_cylinder_add(vertices=8, radius=0.5, depth=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.smart_project(angle_limit=1.1519173, island_margin=0.01)
bpy.ops.uv.average_islands_scale()
bpy.ops.uv.pack_islands(shape_method='AABB', rotate=False, margin=0.02)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cone_add(vertices=8, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.smart_project(angle_limit=1.1519173, island_margin=0.01)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.export_scene.gltf(filepath='out/props.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/props.glb')
bpy.ops.export_scene.gltf(filepath='out/props_rt.glb')
