# M8 stabilization round 6: `mesh.vertex_colors.remove(layer)` removes the BYTE_COLOR CORNER
# attribute; an active or default colour attribute that named it moves to the colour attribute
# now at its index, clamped to the last (unlike `color_attributes.remove`, which keeps both
# names); a layer named by index works; the last layer gone leaves the FLOAT_COLOR one. It
# raised NotImplementedError before. Names are moved into material and object names so that
# they are compared; objects are renamed last. Round 9: the last colour layer gone leaves both
# names '' but set, so a later `vertex_colors.new()` or `color_attributes.new()` is neither
# active nor default (new materials carry the names).
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.meshes['Cube'].vertex_colors.new(name='A')
bpy.data.meshes['Cube'].vertex_colors.new(name='B')
bpy.data.meshes['Cube'].vertex_colors.new(name='C')
bpy.data.meshes['Cube'].color_attributes.new('F', 'FLOAT_COLOR', 'POINT')
bpy.data.meshes['Cube'].vertex_colors.active_index = 2
bpy.data.meshes['Cube'].vertex_colors.remove(bpy.data.meshes['Cube'].vertex_colors['C'])
bpy.data.materials['Material'].name = bpy.data.meshes['Cube'].color_attributes.active_color_name + '-' + bpy.data.meshes['Cube'].attributes.default_color_name
bpy.data.objects['Cube'].location.x = bpy.data.meshes['Cube'].vertex_colors.active_index
bpy.data.meshes['Cube'].vertex_colors.active_index = 1
bpy.data.meshes['Cube'].vertex_colors.remove(bpy.data.meshes['Cube'].vertex_colors[0])
bpy.data.objects['Cube'].location.y = bpy.data.meshes['Cube'].vertex_colors.active_index
bpy.data.objects['Light'].name = bpy.data.meshes['Cube'].color_attributes.active_color_name + '-' + bpy.data.meshes['Cube'].attributes.default_color_name
bpy.data.meshes['Cube'].vertex_colors.remove(bpy.data.meshes['Cube'].color_attributes['F'])
bpy.data.meshes['Cube'].vertex_colors.remove(bpy.data.meshes['Cube'].vertex_colors['B'])
bpy.data.objects['Cube'].location.z = bpy.data.meshes['Cube'].color_attributes.active_color_index
bpy.data.objects['Camera'].name = bpy.data.meshes['Cube'].color_attributes.active_color_name + '-' + bpy.data.meshes['Cube'].attributes.default_color_name
bpy.data.meshes['Cube'].vertex_colors.new(name='D')
bpy.data.objects['Cube'].rotation_euler.x = bpy.data.meshes['Cube'].vertex_colors.active_index
bpy.data.objects['Cube'].rotation_euler.y = bpy.data.meshes['Cube'].color_attributes.active_color_index
bpy.data.objects['Cube'].rotation_euler.z = bpy.data.meshes['Cube'].vertex_colors['D'].active_render
bpy.data.objects['Cube'].scale.x = bpy.data.meshes['Cube'].vertex_colors['D'].active
bpy.data.materials.new('N-' + bpy.data.meshes['Cube'].attributes.active_color_name + '-' + bpy.data.meshes['Cube'].attributes.default_color_name)
bpy.data.meshes['Cube'].vertex_colors.remove(bpy.data.meshes['Cube'].vertex_colors['D'])
bpy.data.meshes['Cube'].color_attributes.new('P', 'FLOAT_COLOR', 'POINT')
bpy.data.objects['Cube'].scale.y = bpy.data.meshes['Cube'].color_attributes.active_color_index + 2
bpy.data.objects['Cube'].scale.z = bpy.data.meshes['Cube'].color_attributes.render_color_index + 2
bpy.data.materials.new('P-' + bpy.data.meshes['Cube'].attributes.active_color_name + '-' + bpy.data.meshes['Cube'].attributes.default_color_name)
bpy.data.meshes['Cube'].vertex_colors.active_index = 0
bpy.data.meshes['Cube'].color_attributes.active_color = bpy.data.meshes['Cube'].color_attributes['P']
bpy.data.materials.new('Q-' + bpy.data.meshes['Cube'].attributes.active_color_name + '-' + bpy.data.meshes['Cube'].attributes.default_color_name)
