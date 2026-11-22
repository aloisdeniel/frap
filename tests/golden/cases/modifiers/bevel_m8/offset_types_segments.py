# WP 8.2: BEVEL WIDTH, DEPTH, PERCENT and ABSOLUTE amounts with three segments on
# non-uniform boxes.
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002,evaluated:Cube.003 tier=T1
import bpy
bpy.ops.object.delete()
bpy.ops.mesh.primitive_cube_add(location=(0, 0, 0), scale=(1, 2, 0.5))
bpy.ops.object.transform_apply(scale=True)
bpy.data.objects['Cube'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube'].modifiers['Bevel'].offset_type = 'WIDTH'
bpy.data.objects['Cube'].modifiers['Bevel'].width = 0.3
bpy.data.objects['Cube'].modifiers['Bevel'].segments = 3
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0), scale=(1, 2, 0.5))
bpy.ops.object.transform_apply(scale=True)
bpy.data.objects['Cube.001'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube.001'].modifiers['Bevel'].offset_type = 'DEPTH'
bpy.data.objects['Cube.001'].modifiers['Bevel'].width = 0.3
bpy.data.objects['Cube.001'].modifiers['Bevel'].segments = 3
bpy.ops.mesh.primitive_cube_add(location=(6, 0, 0), scale=(1, 2, 0.5))
bpy.ops.object.transform_apply(scale=True)
bpy.data.objects['Cube.002'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube.002'].modifiers['Bevel'].offset_type = 'PERCENT'
bpy.data.objects['Cube.002'].modifiers['Bevel'].width_pct = 15
bpy.data.objects['Cube.002'].modifiers['Bevel'].segments = 3
bpy.ops.mesh.primitive_cube_add(location=(9, 0, 0), scale=(1, 2, 0.5))
bpy.ops.object.transform_apply(scale=True)
bpy.data.objects['Cube.003'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube.003'].modifiers['Bevel'].offset_type = 'ABSOLUTE'
bpy.data.objects['Cube.003'].modifiers['Bevel'].width = 0.3
bpy.data.objects['Cube.003'].modifiers['Bevel'].segments = 3
