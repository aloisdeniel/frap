# WP 3.9: shading for export. A UV sphere shaded smooth (shared vertices), a cylinder shaded
# smooth with its cap edges marked sharp in Edit Mode (split normals along the rim) and a cube
# kept flat; corner normals decide the glTF vertex split. Round trip (§17.2): import detects
# flat and smooth faces from the normals and merges vertices, the re-export matches.
# golden: startup=factory tier=T2 compare=glb:out/shading.glb,glb:out/shading_rt.glb
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, location=(3, 0, 0))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_cylinder_add(vertices=10, location=(-3, 0, 0))
bpy.ops.object.shade_smooth()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.edges_select_sharp(sharpness=1.0)
bpy.ops.mesh.mark_sharp()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.export_scene.gltf(filepath='out/shading.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/shading.glb')
bpy.ops.export_scene.gltf(filepath='out/shading_rt.glb')
