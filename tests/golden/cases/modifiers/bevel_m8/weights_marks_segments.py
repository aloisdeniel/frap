# WP 8.2: BEVEL limit WEIGHT with three segments (weighted offsets), and marks
# and a material index for the new faces with four segments.
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001 tier=T1
import bpy
bpy.data.meshes['Cube'].attributes.new('bevel_weight_edge', 'FLOAT', 'EDGE')
bpy.data.meshes['Cube'].attributes['bevel_weight_edge'].data[0].value = 1.0
bpy.data.meshes['Cube'].attributes['bevel_weight_edge'].data[1].value = 0.5
bpy.data.meshes['Cube'].attributes['bevel_weight_edge'].data[2].value = 1.0
bpy.data.meshes['Cube'].attributes['bevel_weight_edge'].data[7].value = 0.25
bpy.data.objects['Cube'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube'].modifiers['Bevel'].limit_method = 'WEIGHT'
bpy.data.objects['Cube'].modifiers['Bevel'].width = 0.3
bpy.data.objects['Cube'].modifiers['Bevel'].segments = 3
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.data.meshes['Cube.001'].materials.append(bpy.data.materials.new('A'))
bpy.data.meshes['Cube.001'].materials.append(bpy.data.materials.new('B'))
bpy.data.objects['Cube.001'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube.001'].modifiers['Bevel'].width = 0.2
bpy.data.objects['Cube.001'].modifiers['Bevel'].segments = 4
bpy.data.objects['Cube.001'].modifiers['Bevel'].material = 1
bpy.data.objects['Cube.001'].modifiers['Bevel'].mark_seam = True
bpy.data.objects['Cube.001'].modifiers['Bevel'].mark_sharp = True
