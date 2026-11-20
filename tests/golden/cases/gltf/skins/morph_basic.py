# WP 6.8: morph targets. A flat-shaded grid whose shape key bends it (corners that share a
# base normal but not the morphed one split), a smooth UV sphere with keys: a moved ring, a
# single moved vertex (sparse), a muted key, a key relative to another key, a key equal to the
# basis (all-zero target), a key with a value; the mesh's weights and extras.targetNames.
# Exports: default; export_morph_normal=False; export_try_sparse_sk=False;
# export_try_omit_sparse_sk; export_morph=False; export_morph_tangent with export_tangents;
# export_yup=False.
# golden: startup=factory tier=T2 tol=1e-5 compare=glb:out/default.glb,glb:out/no_normals.glb,glb:out/dense.glb,glb:out/omit.glb,glb:out/no_morph.glb,glb:out/tangents.glb,glb:out/zup.glb
import bpy
bpy.data.objects['Cube'].hide_render = True
bpy.data.objects['Cube'].select_set(False)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=3, size=2, location=(-2, 0, 0))
bpy.data.objects['Grid'].shape_key_add(name='Basis')
bpy.data.objects['Grid'].shape_key_add(name='Bend')
bpy.data.objects['Grid'].data.shape_keys.key_blocks['Bend'].data[0].co = (-1, -1, 0.5)
bpy.data.objects['Grid'].data.shape_keys.key_blocks['Bend'].data[1].co = (-0.33, -1, 0.3)
bpy.data.objects['Grid'].data.shape_keys.key_blocks['Bend'].data[5].co = (0.33, 0, 0.4)
bpy.data.objects['Grid'].data.shape_keys.key_blocks['Bend'].value = 0.25
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=6, radius=1.0, location=(2, 0, 0))
bpy.ops.object.shade_smooth()
bpy.data.objects['Sphere'].shape_key_add(name='Basis')
bpy.data.objects['Sphere'].shape_key_add(name='Ring')
bpy.data.objects['Sphere'].data.shape_keys.key_blocks['Ring'].data[8].co = (0.9, 0.0, 0.2)
bpy.data.objects['Sphere'].data.shape_keys.key_blocks['Ring'].data[9].co = (0.6, 0.6, 0.2)
bpy.data.objects['Sphere'].data.shape_keys.key_blocks['Ring'].data[10].co = (0.0, 0.9, 0.2)
bpy.data.objects['Sphere'].data.shape_keys.key_blocks['Ring'].data[11].co = (-0.6, 0.6, 0.2)
bpy.data.objects['Sphere'].shape_key_add(name='Tip')
bpy.data.objects['Sphere'].data.shape_keys.key_blocks['Tip'].data[0].co = (0.0, 0.0, 1.6)
bpy.data.objects['Sphere'].shape_key_add(name='Muted')
bpy.data.objects['Sphere'].data.shape_keys.key_blocks['Muted'].data[3].co = (0.0, 0.0, 3.0)
bpy.data.objects['Sphere'].data.shape_keys.key_blocks['Muted'].mute = True
bpy.data.objects['Sphere'].shape_key_add(name='OnRing')
bpy.data.objects['Sphere'].data.shape_keys.key_blocks['OnRing'].relative_key = bpy.data.objects['Sphere'].data.shape_keys.key_blocks['Ring']
bpy.data.objects['Sphere'].data.shape_keys.key_blocks['OnRing'].data[20].co = (0.5, 0.5, -0.5)
bpy.data.objects['Sphere'].shape_key_add(name='Same')
bpy.data.objects['Sphere'].data.shape_keys.key_blocks['Tip'].value = 0.7
bpy.data.objects['Sphere'].data.shape_keys.key_blocks['Ring'].value = 1.0
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.export_scene.gltf(filepath='out/default.glb')
bpy.ops.export_scene.gltf(filepath='out/no_normals.glb', export_morph_normal=False)
bpy.ops.export_scene.gltf(filepath='out/dense.glb', export_try_sparse_sk=False)
bpy.ops.export_scene.gltf(filepath='out/omit.glb', export_try_omit_sparse_sk=True)
bpy.ops.export_scene.gltf(filepath='out/no_morph.glb', export_morph=False)
bpy.ops.export_scene.gltf(filepath='out/tangents.glb', export_tangents=True, export_morph_tangent=True)
bpy.ops.export_scene.gltf(filepath='out/zup.glb', export_yup=False)
