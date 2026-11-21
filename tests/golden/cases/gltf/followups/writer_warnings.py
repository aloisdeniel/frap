# WP 7.7: the meshes behind the add-on's writer warnings (reported by frap as WARNINGs through
# MeshExportCx::warn; the GLBs are compared here, the messages by the frap-io-gltf unit tests):
# the default cylinder (n-gon caps) with export_tangents ("Cylinder: Could not calculate
# tangents. Please try to triangulate the mesh first.", no TANGENT), a cube with an active
# colour attribute its material does not read ("The active Vertex Color will not be exported,
# as it is not used in the node tree of the material"), and a second cube whose two materials read
# two different colour attributes ("glTF specification does not allow this case (multiple
# materials with different Vertex Color)": only the first set is written).
# golden: startup=factory tier=T2 compare=glb:out/tangents.glb,glb:out/colors.glb glb_structure
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(3, 0, 0))
bpy.ops.export_scene.gltf(filepath='out/tangents.glb', export_tangents=True, use_selection=True)
bpy.data.meshes['Cube'].color_attributes.new('Col', 'BYTE_COLOR', 'CORNER')
bpy.ops.mesh.primitive_cube_add(location=(0, 3, 0))
bpy.context.object.data.color_attributes.new('A', 'FLOAT_COLOR', 'POINT')
bpy.context.object.data.color_attributes.new('B', 'BYTE_COLOR', 'CORNER')
bpy.context.object.data.color_attributes['B'].data[0].color = (1.0, 0.0, 0.0, 1.0)
bpy.context.object.data.materials.append(bpy.data.materials.new('UsesA'))
bpy.data.materials['UsesA'].node_tree.nodes.new('ShaderNodeVertexColor')
bpy.data.materials['UsesA'].node_tree.nodes['Color Attribute'].layer_name = 'A'
bpy.data.materials['UsesA'].node_tree.links.new(bpy.data.materials['UsesA'].node_tree.nodes['Color Attribute'].outputs['Color'], bpy.data.materials['UsesA'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.context.object.data.materials.append(bpy.data.materials.new('UsesB'))
bpy.data.materials['UsesB'].node_tree.nodes.new('ShaderNodeVertexColor')
bpy.data.materials['UsesB'].node_tree.nodes['Color Attribute'].layer_name = 'B'
bpy.data.materials['UsesB'].node_tree.links.new(bpy.data.materials['UsesB'].node_tree.nodes['Color Attribute'].outputs['Color'], bpy.data.materials['UsesB'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.context.object.data.polygons[0].material_index = 1
bpy.data.objects['Cylinder'].hide_set(True)
bpy.ops.export_scene.gltf(filepath='out/colors.glb', use_visible=True, export_all_vertex_colors=False)
