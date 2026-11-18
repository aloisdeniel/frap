# WP 4.5: BEVEL `width` and `width_pct` are one value (`BevelModifierData::value`): PERCENT then
# `width = 30` bevels 30 %, `width_pct = 0.25` with the default OFFSET bevels by 0.25. Weight
# attributes on the other domain are adapted in their own type before the float conversion: a
# BOOLEAN `bevel_weight_edge` on POINT is the AND of the edge's vertices, a BOOLEAN
# `bevel_weight_vert` on EDGE the OR of the vertex's edges, INT values mix with rounding.
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002,evaluated:Cube.003,evaluated:Cube.004 tier=T2
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube'].modifiers['Bevel'].offset_type = 'PERCENT'
bpy.data.objects['Cube'].modifiers['Bevel'].width = 30
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.data.objects['Cube.001'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube.001'].modifiers['Bevel'].width_pct = 0.25
bpy.ops.mesh.primitive_cube_add(location=(6, 0, 0))
bpy.data.meshes['Cube.002'].attributes.new('bevel_weight_edge', 'BOOLEAN', 'POINT')
bpy.data.meshes['Cube.002'].attributes['bevel_weight_edge'].data[0].value = True
bpy.data.meshes['Cube.002'].attributes['bevel_weight_edge'].data[1].value = True
bpy.data.meshes['Cube.002'].attributes['bevel_weight_edge'].data[2].value = True
bpy.data.objects['Cube.002'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube.002'].modifiers['Bevel'].limit_method = 'WEIGHT'
bpy.data.objects['Cube.002'].modifiers['Bevel'].width = 0.3
bpy.ops.mesh.primitive_cube_add(location=(9, 0, 0))
bpy.data.meshes['Cube.003'].attributes.new('bevel_weight_vert', 'BOOLEAN', 'EDGE')
bpy.data.meshes['Cube.003'].attributes['bevel_weight_vert'].data[0].value = True
bpy.data.objects['Cube.003'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube.003'].modifiers['Bevel'].limit_method = 'WEIGHT'
bpy.data.objects['Cube.003'].modifiers['Bevel'].affect = 'VERTICES'
bpy.data.objects['Cube.003'].modifiers['Bevel'].width = 0.3
bpy.ops.mesh.primitive_cube_add(location=(12, 0, 0))
bpy.data.meshes['Cube.004'].attributes.new('bevel_weight_edge', 'INT', 'POINT')
bpy.data.meshes['Cube.004'].attributes['bevel_weight_edge'].data[0].value = 1
bpy.data.meshes['Cube.004'].attributes['bevel_weight_edge'].data[1].value = 2
bpy.data.objects['Cube.004'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube.004'].modifiers['Bevel'].limit_method = 'WEIGHT'
bpy.data.objects['Cube.004'].modifiers['Bevel'].width = 0.2
