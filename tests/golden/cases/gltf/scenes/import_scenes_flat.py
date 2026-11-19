# WP 3.10: importing a multi-scene file with `import_scene_as_collection=False`: the default
# glTF scene's objects go to the active collection of the window's scene, every other glTF
# scene becomes a new Blender scene whose scene collection holds its objects.
# golden: startup=factory tier=T1 order=canonical compare=objects,context tol=1e-5
import bpy
bpy.data.scenes.new('S2')
bpy.context.window.scene = bpy.data.scenes['S2']
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(0, 0, 2))
bpy.context.window.scene = bpy.data.scenes['Scene']
bpy.ops.export_scene.gltf(filepath='out/two.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/two.glb', import_scene_as_collection=False)
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.scenes[0].name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.scenes[1].name, None))
bpy.data.scenes['S2'].objects['Cylinder']
bpy.context.window.scene = bpy.data.scenes['S2']
bpy.context.window.scene = bpy.data.scenes['Scene']
