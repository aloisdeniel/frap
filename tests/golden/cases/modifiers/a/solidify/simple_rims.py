# WP 4.2: SOLIDIFY "Simple" rims: an open tube (no caps) with offset 0 and flipped normals; a
# grid with `use_rim_only` (no shell: only the boundary vertices are copied); a grid without
# rim fill.
# golden: startup=empty compare=evaluated:Cylinder,evaluated:Grid,evaluated:Grid.001 tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8, end_fill_type='NOTHING')
bpy.data.objects['Cylinder'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Cylinder'].modifiers['Solidify'].thickness = 0.3
bpy.data.objects['Cylinder'].modifiers['Solidify'].offset = 0
bpy.data.objects['Cylinder'].modifiers['Solidify'].use_flip_normals = True
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=2, location=(3, 0, 0))
bpy.data.objects['Grid'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Grid'].modifiers['Solidify'].thickness = 0.25
bpy.data.objects['Grid'].modifiers['Solidify'].offset = 0.4
bpy.data.objects['Grid'].modifiers['Solidify'].use_rim_only = True
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=2, location=(-3, 0, 0))
bpy.data.objects['Grid.001'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Grid.001'].modifiers['Solidify'].thickness = -0.2
bpy.data.objects['Grid.001'].modifiers['Solidify'].use_rim = False
