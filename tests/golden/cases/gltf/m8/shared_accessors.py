# WP 8.12: `export_shared_accessors`: the triangle primitives of a mesh share one set of vertex
# accessors (POSITION, NORMAL, TEXCOORD_0, COLOR_0, morph targets; the corners of every
# material deduplicated together, `primitive_creation_shared`), each primitive keeping its own
# indices; loose edges and points stay separate primitives with their own attributes. A mesh
# used by two objects with different materials shares the accessors as well (the primitive
# cache is keyed by the mesh).
# golden: startup=factory tier=T2 compare=glb:out/shared.glb,glb:out/plain.glb glb_structure
import bpy
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Red'))
bpy.data.meshes['Cube'].polygons[0].material_index = 1
bpy.data.meshes['Cube'].polygons[3].material_index = 1
bpy.data.meshes['Cube'].color_attributes.new('Col', 'BYTE_COLOR', 'CORNER')
bpy.data.meshes['Cube'].color_attributes['Col'].data[2].color = (1.0, 0.0, 0.0, 1.0)
bpy.data.objects['Cube'].shape_key_add(name='Basis')
bpy.data.objects['Cube'].shape_key_add(name='Up')
bpy.data.meshes['Cube'].shape_keys.key_blocks['Up'].data[0].co = (1.0, 1.0, 2.0)
bpy.data.objects.new('Cube2', bpy.data.meshes['Cube'])
bpy.data.collections['Collection'].objects.link(bpy.data.objects['Cube2'])
bpy.data.objects['Cube2'].location = (4.0, 0.0, 0.0)
bpy.data.objects['Cube2'].material_slots[1].link = 'OBJECT'
bpy.data.objects['Cube2'].material_slots[1].material = bpy.data.materials.new('Blue')
bpy.data.meshes.new('Loose')
bpy.data.meshes['Loose'].from_pydata([(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0), (2, 3, 0), (3, 3, 0), (2, 2, 1)], [(4, 5)], [(0, 1, 2, 3)])
bpy.data.objects.new('Loose', bpy.data.meshes['Loose'])
bpy.data.collections['Collection'].objects.link(bpy.data.objects['Loose'])
bpy.data.objects['Loose'].location = (0.0, 4.0, 0.0)
bpy.ops.export_scene.gltf(filepath='out/shared.glb', export_shared_accessors=True, use_mesh_edges=True, use_mesh_vertices=True, export_all_vertex_colors=True, export_vertex_color='ACTIVE')
bpy.ops.export_scene.gltf(filepath='out/plain.glb', use_mesh_edges=True, use_mesh_vertices=True, export_vertex_color='ACTIVE')
