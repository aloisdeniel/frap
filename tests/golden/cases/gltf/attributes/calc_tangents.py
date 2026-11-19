# WP 3.7: Mesh.calc_tangents(uvmap) fills MeshLoop.tangent / bitangent_sign / bitangent
# (MikkTSpace on the faces, corner normals, the active UV map when uvmap is omitted); the
# values are copied into object transforms so the objects dump compares them. A smooth
# curved patch of quads and a triangle with one mirrored face checks the sign; a second UV
# map named by uvmap= gives other tangents. An explicit uvmap='' or an unknown map, a mesh
# without UV map and a mesh with an n-gon raise Blender's RuntimeErrors (Blender also leaves
# a zeroed tangent layer behind, frap rolls the failing statement back: not compared).
# golden: startup=factory tier=T1 compare=objects tol=1e-5
import bpy
bpy.data.meshes['Cube'].loops[0].tangent
bpy.data.meshes['Cube'].calc_tangents()
bpy.data.objects['Cube'].location = bpy.data.meshes['Cube'].loops[0].tangent
bpy.data.objects['Light'].location = bpy.data.meshes['Cube'].loops[13].bitangent
bpy.data.objects['Camera'].scale = (bpy.data.meshes['Cube'].loops[9].bitangent_sign, bpy.data.meshes['Cube'].loops[22].bitangent_sign, 2.0)
bpy.data.meshes.new('Patch')
bpy.data.meshes['Patch'].from_pydata([(0, 0, 0), (1, 0, 0.2), (2, 0, 0), (0, 1, 0.3), (1, 1, 0.6), (2, 1, 0.2), (0, 2, 0), (1, 2, 0.1), (2, 2, 0), (3, 1, 0.5)], [], [(0, 1, 4, 3), (1, 2, 5, 4), (3, 4, 7, 6), (4, 5, 8, 7), (2, 9, 5)])
bpy.data.meshes['Patch'].shade_smooth()
bpy.data.meshes['Patch'].uv_layers.new(name='UVMap')
bpy.data.meshes['Patch'].uv_layers['UVMap'].data.foreach_set('uv', (0, 0, 0.5, 0, 0.5, 0.5, 0, 0.5, 0.5, 0, 1, 0, 1, 0.5, 0.5, 0.5, 0, 0.5, 0.5, 0.5, 0.5, 1, 0, 1, 1, 0.5, 0.5, 0.5, 0.5, 1, 1, 1, 0.1, 0.3, 0.9, 0.2, 0.4, 0.8))
bpy.data.meshes['Patch'].uv_layers.new(name='Other')
bpy.data.meshes['Patch'].uv_layers['Other'].data.foreach_set('uv', (0, 0, 0, 0.5, 0.5, 0.5, 0.5, 0, 0, 0.5, 0, 1, 0.5, 1, 0.5, 0.5, 0.5, 0, 0.5, 0.5, 1, 0.5, 1, 0, 0.5, 0.5, 0.5, 1, 1, 1, 1, 0.5, 0.2, 0.2, 0.3, 0.9, 0.7, 0.1))
bpy.data.objects.new('Patch', bpy.data.meshes['Patch'])
bpy.data.objects.new('Patch2', bpy.data.meshes['Patch'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Patch'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Patch2'])
bpy.data.meshes['Patch'].calc_tangents()
bpy.data.objects['Patch'].location = bpy.data.meshes['Patch'].loops[6].tangent
bpy.data.objects['Patch'].scale = bpy.data.meshes['Patch'].loops[17].bitangent
bpy.data.objects['Patch'].rotation_euler = (bpy.data.meshes['Patch'].loops[6].bitangent_sign, bpy.data.meshes['Patch'].loops[17].bitangent_sign, bpy.data.meshes['Patch'].loops[19].bitangent_sign)
bpy.data.meshes['Patch'].calc_tangents(uvmap='Other')
bpy.data.objects['Patch2'].location = bpy.data.meshes['Patch'].loops[10].tangent
bpy.data.objects['Patch2'].scale = bpy.data.meshes['Patch'].loops[18].tangent
bpy.data.objects['Patch2'].rotation_euler = bpy.data.meshes['Patch'].loops[15].bitangent
bpy.data.meshes['Cube'].calc_tangents(uvmap='')
bpy.data.meshes['Cube'].calc_tangents(uvmap='nope')
bpy.ops.mesh.primitive_circle_add(vertices=5, fill_type='NGON')
bpy.data.meshes['Circle'].calc_tangents()
bpy.data.meshes['Circle'].uv_layers.remove(bpy.data.meshes['Circle'].uv_layers[0])
bpy.data.meshes['Circle'].calc_tangents()
