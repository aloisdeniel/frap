# WP 7.2: Draco with every attribute kind. The factory cube with a corner colour (COLOR_0 as
# normalized UNSIGNED_SHORT: an integer attribute, which Draco never quantizes) and tangents
# (TANGENT, a GENERIC float attribute: export_draco_generic_quantization); a grid with shape
# keys (morph targets: the primitive is encoded with preserve_triangle_order, sequentially, and
# the target accessors keep their buffer views, after the Draco view); a mesh with loose edges
# and loose points under use_mesh_edges/use_mesh_vertices (the LINES and POINTS primitives are
# not encoded); a skinned sphere (JOINTS_0 as UNSIGNED_BYTE, WEIGHTS_0 as FLOAT). A light comes
# first in the scene: the Draco extension is still declared first (`add_draco_extension` runs
# before the traversal). Each file is imported back.
# golden: startup=factory tier=T2 tol=1e-5 order=canonical compare=glb:out/attrs.glb,glb:out/generic8.glb,glb:out/attrs_plain.glb,glb:out/skin.glb,glb:out/skin_plain.glb,mesh:Cube.001,mesh:Grid.001,mesh:Loose.001,mesh:Cube.002,mesh:Grid.002 glb_structure
import bpy
bpy.data.objects['Light'].name = 'A_Light'
bpy.data.meshes['Cube'].color_attributes.new('Col', 'BYTE_COLOR', 'CORNER')
bpy.data.meshes['Cube'].color_attributes['Col'].data[0].color = (1.0, 0.0, 0.0, 1.0)
bpy.data.meshes['Cube'].color_attributes['Col'].data[5].color = (0.2, 0.4, 0.6, 0.5)
bpy.data.meshes['Cube'].color_attributes['Col'].data[13].color = (0.0, 0.0, 1.0, 0.25)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=3, size=2, location=(-3, 0, 0))
bpy.data.objects['Grid'].shape_key_add(name='Basis')
bpy.data.objects['Grid'].shape_key_add(name='Bend')
bpy.data.objects['Grid'].data.shape_keys.key_blocks['Bend'].data[0].co = (-1, -1, 0.5)
bpy.data.objects['Grid'].data.shape_keys.key_blocks['Bend'].data[5].co = (0.33, 0, 0.4)
bpy.data.objects['Grid'].data.shape_keys.key_blocks['Bend'].value = 0.25
bpy.data.meshes.new('Loose')
bpy.data.meshes['Loose'].from_pydata([(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0), (2, 0, 0), (3, 0, 1), (3, 2, 1), (5, 5, 5)], [(4, 5), (5, 6)], [(0, 1, 2, 3)])
bpy.data.objects.new('Loose', bpy.data.meshes['Loose'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Loose'])
bpy.data.objects['Loose'].location = (0, -4, 0)
bpy.ops.export_scene.gltf(filepath='out/attrs.glb', export_draco_mesh_compression_enable=True, export_tangents=True, use_mesh_edges=True, use_mesh_vertices=True, export_lights=True)
bpy.ops.export_scene.gltf(filepath='out/generic8.glb', export_draco_mesh_compression_enable=True, export_tangents=True, export_draco_generic_quantization=8, export_draco_color_quantization=4, export_draco_mesh_compression_level=10)
bpy.ops.export_scene.gltf(filepath='out/attrs_plain.glb', export_tangents=True, use_mesh_edges=True, use_mesh_vertices=True, export_lights=True)
bpy.ops.import_scene.gltf(filepath='out/attrs.glb')
bpy.ops.import_scene.gltf(filepath='out/generic8.glb')
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.armature_add(location=(0, 0, 3))
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, radius=1.0, location=(0, 0, 4))
bpy.data.objects['Sphere'].vertex_groups.new(name='Bone')
bpy.data.objects['Sphere'].vertex_groups['Bone'].add([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15], 1.0, 'REPLACE')
bpy.data.objects['Sphere'].vertex_groups['Bone'].add([16, 17, 18, 19, 20, 21], 0.4, 'REPLACE')
bpy.data.objects['Sphere'].modifiers.new(name='Armature', type='ARMATURE')
bpy.data.objects['Sphere'].modifiers['Armature'].object = bpy.data.objects['Armature']
bpy.data.objects['Sphere'].parent = bpy.data.objects['Armature']
bpy.ops.export_scene.gltf(filepath='out/skin.glb', export_draco_mesh_compression_enable=True)
bpy.ops.export_scene.gltf(filepath='out/skin_plain.glb')
