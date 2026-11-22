# WP 8.12: `export_attributes`: mesh attributes named `_…` or `KHR_…` become glTF attributes
# named in upper case, as stored (no Y-up, no V flip): FLOAT, INT, INT8 and BOOLEAN as FLOAT
# SCALAR, FLOAT2, FLOAT_VECTOR, FLOAT_COLOR and QUATERNION as FLOAT vectors, BYTE_COLOR as
# normalized UNSIGNED_SHORT VEC4 (linear), FLOAT4X4 as MAT4; POINT read per vertex, FACE spread
# over the face's corners, CORNER per corner (splitting vertices); EDGE attributes, other names
# and STRING attributes are not exported. Loose edges and points carry the POINT attributes.
# Without the option nothing is written.
# golden: startup=factory tier=T2 compare=glb:out/attrs.glb,glb:out/none.glb glb_structure
import bpy
bpy.data.meshes['Cube'].attributes.new('_temperature', 'FLOAT', 'POINT')
bpy.data.meshes['Cube'].attributes['_temperature'].data.foreach_set('value', [0.5, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5])
bpy.data.meshes['Cube'].attributes.new('_id', 'INT', 'FACE')
bpy.data.meshes['Cube'].attributes['_id'].data.foreach_set('value', [3, -1, 4, 1, -5, 9])
bpy.data.meshes['Cube'].attributes.new('KHR_small', 'INT8', 'POINT')
bpy.data.meshes['Cube'].attributes['KHR_small'].data.foreach_set('value', [1, -2, 3, -4, 5, -6, 7, -8])
bpy.data.meshes['Cube'].attributes.new('_flag', 'BOOLEAN', 'FACE')
bpy.data.meshes['Cube'].attributes['_flag'].data.foreach_set('value', [True, False, True, False, False, True])
bpy.data.meshes['Cube'].attributes.new('_vec', 'FLOAT_VECTOR', 'POINT')
bpy.data.meshes['Cube'].attributes['_vec'].data.foreach_set('vector', [0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 2.75, 3.0, 3.25, 3.5, 3.75, 4.0, 4.25, 4.5, 4.75, 5.0, 5.25, 5.5, 5.75])
bpy.data.meshes['Cube'].attributes.new('_uvish', 'FLOAT2', 'CORNER')
bpy.data.meshes['Cube'].attributes['_uvish'].data.foreach_set('vector', [0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.0, 0.125, 0.25, 0.375, 0.5, 0.625])
bpy.data.meshes['Cube'].attributes.new('_fcol', 'FLOAT_COLOR', 'FACE')
bpy.data.meshes['Cube'].attributes['_fcol'].data.foreach_set('color', [0.0, 0.2, 0.4, 0.6, 0.8, 0.0, 0.2, 0.4, 0.6, 0.8, 0.0, 0.2, 0.4, 0.6, 0.8, 0.0, 0.2, 0.4, 0.6, 0.8, 0.0, 0.2, 0.4, 0.6])
bpy.data.meshes['Cube'].attributes.new('_bcol', 'BYTE_COLOR', 'POINT')
bpy.data.meshes['Cube'].attributes['_bcol'].data.foreach_set('color', [0.0, 0.5, 1.0, 0.0, 0.5, 1.0, 0.0, 0.5, 1.0, 0.0, 0.5, 1.0, 0.0, 0.5, 1.0, 0.0, 0.5, 1.0, 0.0, 0.5, 1.0, 0.0, 0.5, 1.0, 0.0, 0.5, 1.0, 0.0, 0.5, 1.0, 0.0, 0.5])
bpy.data.meshes['Cube'].attributes.new('_rot', 'QUATERNION', 'POINT')
bpy.data.meshes['Cube'].attributes.new('_mat', 'FLOAT4X4', 'FACE')
bpy.data.meshes['Cube'].attributes['_mat'].data.foreach_set('value', [0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5])
bpy.data.meshes['Cube'].attributes.new('_edge', 'FLOAT', 'EDGE')
bpy.data.meshes['Cube'].attributes.new('_name', 'STRING', 'POINT')
bpy.data.meshes['Cube'].attributes.new('plain', 'FLOAT', 'POINT')
bpy.data.meshes.new('Loose')
bpy.data.meshes['Loose'].from_pydata([(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0), (2, 3, 0), (3, 3, 0), (2, 2, 1)], [(4, 5)], [(0, 1, 2, 3)])
bpy.data.meshes['Loose'].attributes.new('_w', 'FLOAT', 'POINT')
bpy.data.meshes['Loose'].attributes['_w'].data.foreach_set('value', [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
bpy.data.meshes['Loose'].attributes.new('_c', 'FLOAT', 'CORNER')
bpy.data.objects.new('Loose', bpy.data.meshes['Loose'])
bpy.data.collections['Collection'].objects.link(bpy.data.objects['Loose'])
bpy.ops.export_scene.gltf(filepath='out/attrs.glb', export_attributes=True, use_mesh_edges=True, use_mesh_vertices=True)
bpy.ops.export_scene.gltf(filepath='out/none.glb')
