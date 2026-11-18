# WP 3.10: importing a multi-scene file: the default glTF scene goes into the window's scene
# (one collection per glTF scene under the active collection, the other scenes' collections
# excluded), every other glTF scene becomes a new Blender scene with Blender's default
# settings. Scene names and settings are read back into empties.
# golden: startup=factory tier=T1 order=canonical compare=objects,context tol=1e-5
import bpy
bpy.data.scenes.new('S2')
bpy.context.window.scene = bpy.data.scenes['S2']
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(0, 0, 2))
bpy.context.window.scene = bpy.data.scenes['Scene']
bpy.ops.export_scene.gltf(filepath='out/two.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/two.glb')
bpy.context.scene.name
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.scenes[0].name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.scenes[1].name, None))
bpy.data.objects['S2'].location = (bpy.data.scenes['S2'].render.resolution_x, bpy.data.scenes['S2'].frame_end, bpy.data.scenes['S2'].display.shading.xray_alpha_wireframe)
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.scenes['S2'].render.filepath, None))
bpy.context.view_layer.layer_collection.children['S2'].exclude
bpy.context.window.scene = bpy.data.scenes['S2']
bpy.context.scene.collection.objects.link(bpy.data.objects.new('InS2', None))
bpy.context.window.scene = bpy.data.scenes['Scene']
