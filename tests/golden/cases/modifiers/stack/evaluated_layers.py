# M8 stabilization round 4: the mesh-layer hooks of an evaluated mesh (a SUBSURF Cube with two
# UV maps, the second active, and a colour attribute) read the evaluated mesh: uv_layers.active
# and active_index, the active layer's uv vectors and data, color_attributes' active and
# render indices; they raised "frap internal: not a mesh" before. Values are copied into the
# Light's name and the Camera's transform. attributes.active is only run (frap keeps the
# active attribute by name, Blender by index into the evaluated mesh's own attribute order:
# D-72, D-74).
# golden: startup=factory compare=objects tier=T1
import bpy
bpy.data.meshes['Cube'].uv_layers.new(name='B')
bpy.data.meshes['Cube'].color_attributes.new('Col', 'FLOAT_COLOR', 'CORNER')
bpy.data.meshes['Cube'].uv_layers.active_index = 1
bpy.data.objects['Cube'].modifiers.new(name='S', type='SUBSURF')
bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.uv_layers.active
bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.uv_layers.active_index
bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.attributes.active
bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.attributes.active_index
bpy.data.objects['Light'].name = bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.uv_layers.active.name
bpy.data.objects['Camera'].location.x = bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.uv_layers.active_index
bpy.data.objects['Camera'].location.y = bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.uv_layers.active.uv[7].vector.y
bpy.data.objects['Camera'].location.z = bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.uv_layers.active.data[9].uv.x
bpy.data.objects['Camera'].scale.x = bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.color_attributes.active_color_index
bpy.data.objects['Camera'].scale.y = bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.color_attributes.render_color_index
bpy.data.objects['Camera'].scale.z = bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.uv_layers['UVMap'].uv[20].vector.x
bpy.data.objects['Cube'].to_mesh().uv_layers.active_index
bpy.data.objects['Camera'].rotation_euler.x = bpy.data.objects['Cube'].to_mesh().uv_layers.active_index
