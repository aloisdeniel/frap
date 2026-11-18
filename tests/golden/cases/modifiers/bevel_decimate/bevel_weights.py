# WP 4.5: BEVEL limit method WEIGHT: edges by the `bevel_weight_edge` attribute (offsets scaled
# by the weight, weight 0 not beveled), vertices by `bevel_weight_vert`; without the attribute
# nothing is beveled. The new faces take material index 5 clamped to the last slot (1), with
# seams and sharp edges marked.
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002,evaluated:Cube.003 tier=T2
import bpy
bpy.data.meshes['Cube'].attributes.new('bevel_weight_edge', 'FLOAT', 'EDGE')
bpy.data.meshes['Cube'].attributes['bevel_weight_edge'].data[0].value = 1.0
bpy.data.meshes['Cube'].attributes['bevel_weight_edge'].data[1].value = 0.5
bpy.data.meshes['Cube'].attributes['bevel_weight_edge'].data[2].value = 1.0
bpy.data.objects['Cube'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube'].modifiers['Bevel'].limit_method = 'WEIGHT'
bpy.data.objects['Cube'].modifiers['Bevel'].width = 0.3
bpy.data.objects['Cube'].modifiers['Bevel'].segments = 2
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.data.meshes['Cube.001'].attributes.new('bevel_weight_vert', 'FLOAT', 'POINT')
bpy.data.meshes['Cube.001'].attributes['bevel_weight_vert'].data[0].value = 1.0
bpy.data.meshes['Cube.001'].attributes['bevel_weight_vert'].data[3].value = 0.5
bpy.data.objects['Cube.001'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube.001'].modifiers['Bevel'].limit_method = 'WEIGHT'
bpy.data.objects['Cube.001'].modifiers['Bevel'].affect = 'VERTICES'
bpy.data.objects['Cube.001'].modifiers['Bevel'].width = 0.4
bpy.ops.mesh.primitive_cube_add(location=(6, 0, 0))
bpy.data.objects['Cube.002'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube.002'].modifiers['Bevel'].limit_method = 'WEIGHT'
bpy.ops.mesh.primitive_cube_add(location=(9, 0, 0))
bpy.data.meshes['Cube.003'].materials.append(bpy.data.materials.new('A'))
bpy.data.meshes['Cube.003'].materials.append(bpy.data.materials.new('B'))
bpy.data.objects['Cube.003'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube.003'].modifiers['Bevel'].material = 5
bpy.data.objects['Cube.003'].modifiers['Bevel'].width = 0.2
bpy.data.objects['Cube.003'].modifiers['Bevel'].mark_seam = True
bpy.data.objects['Cube.003'].modifiers['Bevel'].mark_sharp = True
