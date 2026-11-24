# M8 stabilization round 5: mesh.vertex_colors.active and active_index (LoopColors) map to the
# active colour attribute when it is a BYTE_COLOR layer on CORNER, None / -1 otherwise (no
# colour attribute, a FLOAT_COLOR or POINT one active); assigning a layer or an index makes it
# the active colour attribute, assigning None or an index outside the layers (7, and -1 since
# round 6: it was clamped to 0) changes nothing; the evaluated mesh answers the same. `active`
# raised NotImplementedError ("needs an RNA get hook") before. Results are moved into object
# transforms and names (the material's name shows in the Cube's material list) so that they
# are compared; objects are renamed last.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.meshes['Cube'].vertex_colors.active.name
bpy.data.objects['Cube'].location.x = bpy.data.meshes['Cube'].vertex_colors.active_index
bpy.data.meshes['Cube'].vertex_colors.new(name='A')
bpy.data.meshes['Cube'].vertex_colors.new(name='B')
bpy.data.materials['Material'].name = bpy.data.meshes['Cube'].vertex_colors.active.name
bpy.data.objects['Cube'].location.y = bpy.data.meshes['Cube'].vertex_colors.active_index
bpy.data.meshes['Cube'].color_attributes.new('F', 'FLOAT_COLOR', 'CORNER')
bpy.data.meshes['Cube'].color_attributes.active_color = bpy.data.meshes['Cube'].color_attributes['F']
bpy.data.meshes['Cube'].vertex_colors.active.name
bpy.data.objects['Cube'].location.z = bpy.data.meshes['Cube'].vertex_colors.active_index
bpy.data.meshes['Cube'].color_attributes.new('P', 'BYTE_COLOR', 'POINT')
bpy.data.meshes['Cube'].color_attributes.active_color_name = 'P'
bpy.data.objects['Cube'].scale.x = bpy.data.meshes['Cube'].vertex_colors.active_index
bpy.data.meshes['Cube'].color_attributes.active_color_name = 'B'
bpy.data.materials['A'].name = bpy.data.meshes['Cube'].vertex_colors.active.name
bpy.data.objects['Cube'].scale.y = bpy.data.meshes['Cube'].vertex_colors.active_index
bpy.data.meshes['Cube'].vertex_colors.active = bpy.data.meshes['Cube'].vertex_colors['A']
bpy.data.objects['Cube'].scale.z = bpy.data.meshes['Cube'].vertex_colors.active_index
bpy.data.meshes['Cube'].vertex_colors.active_index = 1
bpy.data.objects['Camera'].location.x = bpy.data.meshes['Cube'].vertex_colors.active_index
bpy.data.meshes['Cube'].vertex_colors.active_index = 7
bpy.data.meshes['Cube'].vertex_colors.active_index = -1
bpy.data.meshes['Cube'].vertex_colors.active = None
bpy.data.objects['Camera'].location.y = bpy.data.meshes['Cube'].vertex_colors.active_index
bpy.data.meshes['Cube'].color_attributes.remove(bpy.data.meshes['Cube'].color_attributes['A'])
bpy.data.objects['Camera'].location.z = bpy.data.meshes['Cube'].vertex_colors.active_index
bpy.data.objects['Cube'].modifiers.new(name='S', type='SUBSURF')
bpy.data.objects['Camera'].scale.x = bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.vertex_colors.active_index
bpy.data.objects['Light'].name = bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.vertex_colors.active.name
bpy.data.objects['Camera'].name = bpy.data.meshes['Cube'].color_attributes.active_color_name + bpy.data.meshes['Cube'].attributes.default_color_name
