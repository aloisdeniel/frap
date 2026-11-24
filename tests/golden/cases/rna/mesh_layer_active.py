# M2 W0 (M1 leftover): the active-layer members of mesh.uv_layers and mesh.attributes /
# mesh.color_attributes (UVLoopLayers.active/active_index, AttributeGroupMesh.active_color,
# active_color_index, active_color_name, default_color_name, render_color_index): reads,
# pointer and index assignment, out-of-range indices ignored (negative ones too since M8
# stabilization round 7: -1 was clamped to 0, D-53), None clears. Results are moved into
# object transforms and names so that they are compared.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.meshes['Cube'].uv_layers.new(name='B')
bpy.data.objects['Cube'].location.x = bpy.data.meshes['Cube'].uv_layers.active_index
bpy.data.meshes['Cube'].uv_layers.active_index = 1
bpy.data.objects['Cube'].location.y = bpy.data.meshes['Cube'].uv_layers.active_index
bpy.data.objects['Camera'].name = bpy.data.meshes['Cube'].uv_layers.active.name
bpy.data.meshes['Cube'].uv_layers.active_index = 5
bpy.data.objects['Cube'].location.z = bpy.data.meshes['Cube'].uv_layers.active_index
bpy.data.meshes['Cube'].uv_layers.active_index = -1
bpy.data.objects['B'].location.x = bpy.data.meshes['Cube'].uv_layers.active_index
bpy.data.meshes['Cube'].uv_layers.active_index = -5
bpy.data.objects['B'].location.y = bpy.data.meshes['Cube'].uv_layers.active_index
bpy.data.meshes['Cube'].uv_layers.active = bpy.data.meshes['Cube'].uv_layers['UVMap']
bpy.data.objects['Cube'].scale.x = bpy.data.meshes['Cube'].uv_layers.active_index
bpy.data.meshes['Cube'].uv_layers.active = None
bpy.data.objects['Cube'].scale.y = bpy.data.meshes['Cube'].uv_layers.active_index
bpy.data.objects['Cube'].scale.z = bpy.data.meshes['Cube'].attributes.active_color_index
bpy.data.meshes['Cube'].color_attributes.new('Col', 'FLOAT_COLOR', 'POINT')
bpy.data.meshes['Cube'].color_attributes.new('Col2', 'BYTE_COLOR', 'CORNER')
bpy.data.objects['Light'].location.x = bpy.data.meshes['Cube'].attributes.active_color_index
bpy.data.objects['Light'].location.y = bpy.data.meshes['Cube'].attributes.render_color_index
bpy.data.meshes['Cube'].attributes.active_color_index = 1
bpy.data.objects['Light'].location.z = bpy.data.meshes['Cube'].attributes.active_color_index
bpy.data.meshes['Cube'].attributes.active_color = bpy.data.meshes['Cube'].attributes['Col']
bpy.data.objects['Light'].scale.x = bpy.data.meshes['Cube'].color_attributes.active_color_index
bpy.data.meshes['Cube'].attributes.render_color_index = 1
bpy.data.objects['Light'].scale.y = bpy.data.meshes['Cube'].attributes.render_color_index
bpy.data.meshes['Cube'].attributes.default_color_name = 'Col'
bpy.data.objects['Light'].scale.z = bpy.data.meshes['Cube'].attributes.render_color_index
bpy.data.meshes['Cube'].attributes.default_color_name = 'Nope'
bpy.data.objects['Light'].rotation_euler.x = bpy.data.meshes['Cube'].attributes.render_color_index
bpy.data.meshes['Cube'].attributes.active_color_name = 'Col2'
bpy.data.objects['Light'].name = bpy.data.meshes['Cube'].color_attributes.active_color.name
bpy.data.meshes['Cube'].attributes.active_color = None
bpy.data.objects['Cube'].rotation_euler.x = bpy.data.meshes['Cube'].attributes.active_color_index
bpy.data.objects['Cube'].name = bpy.data.meshes['Cube'].attributes.default_color_name + bpy.data.meshes['Cube'].attributes.active_color_name
