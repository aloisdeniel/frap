# WP 4.3: TRIANGULATE quad methods BEAUTY (Suzanne), FIXED with CLIP n-gons (cylinder caps),
# LONGEST_DIAGONAL and FIXED_ALTERNATE with a minimum of 5 corners (only the caps split).
# golden: startup=empty compare=evaluated:Suzanne,evaluated:Cylinder,evaluated:Cylinder.001,evaluated:Sphere tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.data.objects['Suzanne'].modifiers.new(name='M', type='TRIANGULATE')
bpy.data.objects['Suzanne'].modifiers['M'].quad_method = 'BEAUTY'
bpy.ops.mesh.primitive_cylinder_add(vertices=7, location=(3, 0, 0))
bpy.data.objects['Cylinder'].modifiers.new(name='M', type='TRIANGULATE')
bpy.data.objects['Cylinder'].modifiers['M'].quad_method = 'FIXED'
bpy.data.objects['Cylinder'].modifiers['M'].ngon_method = 'CLIP'
bpy.ops.mesh.primitive_cylinder_add(vertices=9, location=(6, 0, 0))
bpy.data.objects['Cylinder.001'].modifiers.new(name='M', type='TRIANGULATE')
bpy.data.objects['Cylinder.001'].modifiers['M'].quad_method = 'FIXED_ALTERNATE'
bpy.data.objects['Cylinder.001'].modifiers['M'].min_vertices = 5
bpy.ops.mesh.primitive_uv_sphere_add(segments=10, ring_count=6, location=(9, 0, 0))
bpy.data.objects['Sphere'].modifiers.new(name='M', type='TRIANGULATE')
bpy.data.objects['Sphere'].modifiers['M'].quad_method = 'LONGEST_DIAGONAL'
