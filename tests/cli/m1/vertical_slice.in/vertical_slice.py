bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete()
mesh.primitive_cylinder_add vertices=16 radius=0.5 depth=2 location=0,0,1
bpy.data.materials.new("Steel")
bpy.data.materials['Steel'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Steel'].diffuse_color = (0.6, 0.6, 0.7, 1.0)
bpy.context.object.data.materials.append(bpy.data.materials['Steel'])
bpy.ops.object.camera_add(location=(4, -4, 3), rotation=(1.1, 0, 0.785))
bpy.context.scene.camera = bpy.data.objects['Camera']
data.scenes['Scene'].render.engine = BLENDER_WORKBENCH
bpy.context.scene.render.filepath = 'out/part.png'
bpy.ops.render.render(write_still=True)
bpy.ops.export_scene.gltf(filepath='out/part.glb')
bpy.ops.wm.save_as_mainfile(filepath='out/part.frap')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/part.glb')
bpy.data.objects['Cylinder'].data.materials[0].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value
bpy.ops.wm.open_mainfile(filepath='out/part.frap')
bpy.data.objects['Cylinder'].location
