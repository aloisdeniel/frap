# WP 4.5: BEVEL options: segments and profile, vertex bevels, limit method NONE with the CUTOFF
# vertex mesh, clamping of a too large amount and no clamp, the WIDTH amount with outer ARC
# miters, the PERCENT amount (width_pct).
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002,evaluated:Cube.003,evaluated:Cube.004,evaluated:Cylinder,evaluated:Suzanne tier=T2
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube'].modifiers['Bevel'].segments = 3
bpy.data.objects['Cube'].modifiers['Bevel'].profile = 0.7
bpy.data.objects['Cube'].modifiers['Bevel'].width = 0.2
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.data.objects['Cube.001'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube.001'].modifiers['Bevel'].affect = 'VERTICES'
bpy.data.objects['Cube.001'].modifiers['Bevel'].width = 0.3
bpy.data.objects['Cube.001'].modifiers['Bevel'].segments = 2
bpy.ops.mesh.primitive_cube_add(location=(6, 0, 0))
bpy.data.objects['Cube.002'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube.002'].modifiers['Bevel'].limit_method = 'NONE'
bpy.data.objects['Cube.002'].modifiers['Bevel'].width = 0.2
bpy.data.objects['Cube.002'].modifiers['Bevel'].segments = 2
bpy.data.objects['Cube.002'].modifiers['Bevel'].vmesh_method = 'CUTOFF'
bpy.ops.mesh.primitive_cube_add(location=(9, 0, 0))
bpy.data.objects['Cube.003'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube.003'].modifiers['Bevel'].width = 1.5
bpy.ops.mesh.primitive_cube_add(location=(12, 0, 0))
bpy.data.objects['Cube.004'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube.004'].modifiers['Bevel'].width = 0.6
bpy.data.objects['Cube.004'].modifiers['Bevel'].use_clamp_overlap = False
bpy.ops.mesh.primitive_cylinder_add(vertices=12, location=(0, 4, 0))
bpy.data.objects['Cylinder'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cylinder'].modifiers['Bevel'].offset_type = 'WIDTH'
bpy.data.objects['Cylinder'].modifiers['Bevel'].width = 0.1
bpy.data.objects['Cylinder'].modifiers['Bevel'].miter_outer = 'MITER_ARC'
bpy.data.objects['Cylinder'].modifiers['Bevel'].segments = 2
bpy.ops.mesh.primitive_monkey_add(location=(0, -4, 0))
bpy.data.objects['Suzanne'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Suzanne'].modifiers['Bevel'].offset_type = 'PERCENT'
bpy.data.objects['Suzanne'].modifiers['Bevel'].width_pct = 20
bpy.data.objects['Suzanne'].modifiers['Bevel'].segments = 2
