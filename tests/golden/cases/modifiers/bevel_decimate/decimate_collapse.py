# WP 4.5: DECIMATE COLLAPSE (the default mode): Suzanne to half its faces (quads joined back
# after the collapse), a cylinder kept triangulated, a grid with X symmetry and a cone with
# Y symmetry; ratio 1 (the default) passes the cube through.
# golden: startup=factory compare=evaluated:Suzanne,evaluated:Cylinder,evaluated:Grid,evaluated:Cone,evaluated:Cube tier=T1
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.ops.mesh.primitive_monkey_add(location=(0, 4, 0))
bpy.data.objects['Suzanne'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Suzanne'].modifiers['Decimate'].ratio = 0.5
bpy.ops.mesh.primitive_cylinder_add(location=(4, 0, 0))
bpy.data.objects['Cylinder'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Cylinder'].modifiers['Decimate'].ratio = 0.3
bpy.data.objects['Cylinder'].modifiers['Decimate'].use_collapse_triangulate = True
bpy.ops.mesh.primitive_grid_add(location=(0, -4, 0))
bpy.data.objects['Grid'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Grid'].modifiers['Decimate'].ratio = 0.25
bpy.data.objects['Grid'].modifiers['Decimate'].use_symmetry = True
bpy.ops.mesh.primitive_cone_add(location=(-4, 0, 0))
bpy.data.objects['Cone'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Cone'].modifiers['Decimate'].ratio = 0.4
bpy.data.objects['Cone'].modifiers['Decimate'].use_symmetry = True
bpy.data.objects['Cone'].modifiers['Decimate'].symmetry_axis = 'Y'
