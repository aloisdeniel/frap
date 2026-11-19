# WP 4.3: WIREFRAME boundary rims with an inward offset on a grid; Suzanne keeping its faces
# with relative thickness, creased wires and a material offset clamped to its one slot.
# golden: startup=empty compare=evaluated:Grid,evaluated:Suzanne tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=3, size=2)
bpy.data.objects['Grid'].modifiers.new(name='M', type='WIREFRAME')
bpy.data.objects['Grid'].modifiers['M'].thickness = 0.1
bpy.data.objects['Grid'].modifiers['M'].offset = -0.5
bpy.data.objects['Grid'].modifiers['M'].use_boundary = True
bpy.ops.mesh.primitive_monkey_add(location=(3, 0, 0))
bpy.data.objects['Suzanne'].modifiers.new(name='M', type='WIREFRAME')
bpy.data.objects['Suzanne'].modifiers['M'].thickness = 0.3
bpy.data.objects['Suzanne'].modifiers['M'].use_replace = False
bpy.data.objects['Suzanne'].modifiers['M'].use_relative_offset = True
bpy.data.objects['Suzanne'].modifiers['M'].use_even_offset = False
bpy.data.objects['Suzanne'].modifiers['M'].use_crease = True
bpy.data.objects['Suzanne'].modifiers['M'].crease_weight = 0.7
bpy.data.objects['Suzanne'].modifiers['M'].material_offset = 1
