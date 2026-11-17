bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))
mesh.primitive_cube_add size=2 location=0,0,1
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.data.objects['Cube'].location
bpy.ops.export_scene.gltf(filepath='out/m0.glb')
