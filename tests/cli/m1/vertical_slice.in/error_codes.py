bpy.data.objects['Cube'].location =
x = 1
cube
bpy.ops.mesh.foo()
bpy.ops.mesh.knife_project()
bpy.context.active_operator
bpy.ops.mesh.primitive_cube_add(sizee=2)
bpy.ops.mesh.primitive_cube_add(size='x')
bpy.ops.mesh.primitive_cube_add(align='FOO')
bpy.ops.mesh.primitive_cube_add(2)
bpy.ops.mesh.primitive_cube_add(location=(1, 2))
bpy.ops.object.mode_set(mode='POSE')
bpy.data.objects['Cube'].foo
bpy.data.objects['Cube'].type = 'EMPTY'
bpy.context.object = None
bpy.data.meshes['Cube'].use_auto_smooth
bpy.data.objects['Nope']
bpy.data.objects[9]
bpy.data.objects['Cube'].name = 1
bpy.data.objects['Cube'].rotation_mode = 'FOO'
bpy.data.objects['Cube'].select_set()
print(1 / 0)
bpy.data.objects['Cube'].location = (1, 2)
bpy.data.objects['Cube'].pass_index
bpy.data.objects.remove(bpy.data.objects['Cube'], do_unlink=False)
bpy.ops.export_scene.gltf(filepath='')
bpy.context.view_layer.objects.active = None
bpy.ops.object.mode_set(mode='OBJECT')
