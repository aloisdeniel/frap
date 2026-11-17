bpy.ops.export_scene.gltf(filepath='')
bpy.ops.import_scene.gltf(filepath='missing.glb')
bpy.ops.wm.open_mainfile(filepath='missing.frap')
bpy.context.scene.camera = None
bpy.ops.render.render()
