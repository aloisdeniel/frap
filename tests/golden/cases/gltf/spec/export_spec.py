# WP 7.4: the spec-complete writer changes the layout and the declarations, never the
# content. The same scene (the factory cube with two materials, keyed motion, a UV sphere
# parented to it, a textured plane) is exported by Blender with its defaults and by frap with
# frap_compat='SPEC' and frap_interleave (#@frap / #@blender lines writing the same files):
# the canonical comparison (decoded accessors, triangle multisets, materials, sampled
# animation) finds them equal although frap's vertex attributes share strided views and its
# indices are UNSIGNED_BYTE (UNSIGNED_SHORT for the 482-vertex sphere). fps_base is 1, so the
# SPEC clock equals the add-on's; with export_cameras the camera's yfov is the same in both.
# golden: startup=factory tier=T2 tol=1e-5 compare=glb:out/scene.glb,glb:out/scene_rt.glb
import bpy
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Red'))
bpy.data.materials['Red'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (1.0, 0.0, 0.0, 1.0)
bpy.data.meshes['Cube'].polygons[2].material_index = 1
bpy.data.objects['Cube'].keyframe_insert('location', frame=1)
bpy.data.objects['Cube'].location = (0.0, 1.0, 2.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=20)
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.5, location=(2.0, 0.0, 0.0))
bpy.data.objects['Sphere'].parent = bpy.data.objects['Cube']
bpy.ops.mesh.primitive_plane_add(size=2.0, location=(0.0, -3.0, 0.0))
bpy.data.images.new('Grid', 4, 4)
bpy.data.objects['Plane'].data.materials.append(bpy.data.materials.new('Tex'))
bpy.data.materials['Tex'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Tex'].node_tree.nodes['Image Texture'].image = bpy.data.images['Grid']
bpy.data.materials['Tex'].node_tree.links.new(bpy.data.materials['Tex'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Tex'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
#@frap    bpy.ops.export_scene.gltf(filepath='out/scene.glb', export_cameras=True, frap_compat='SPEC', frap_interleave=True)
#@blender bpy.ops.export_scene.gltf(filepath='out/scene.glb', export_cameras=True)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/scene.glb')
#@frap    bpy.ops.export_scene.gltf(filepath='out/scene_rt.glb', export_cameras=True, frap_compat='SPEC', frap_interleave=True)
#@blender bpy.ops.export_scene.gltf(filepath='out/scene_rt.glb', export_cameras=True)
