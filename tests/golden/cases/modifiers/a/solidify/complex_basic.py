# WP 4.2: SOLIDIFY "Complex" (NON_MANIFOLD) with the default CONSTRAINTS thickness mode: a
# closed cube gets an inner shell, a plane a shell and a rim, an open tube rims at both ends,
# Suzanne (open eyes) with offset 0.
# golden: startup=empty compare=evaluated:Cube,evaluated:Plane,evaluated:Cylinder,evaluated:Suzanne tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_cube_add()
bpy.data.objects['Cube'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Cube'].modifiers['Solidify'].solidify_mode = 'NON_MANIFOLD'
bpy.data.objects['Cube'].modifiers['Solidify'].thickness = 0.2
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.data.objects['Plane'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Plane'].modifiers['Solidify'].solidify_mode = 'NON_MANIFOLD'
bpy.data.objects['Plane'].modifiers['Solidify'].thickness = 0.1
bpy.ops.mesh.primitive_cylinder_add(vertices=8, end_fill_type='NOTHING', location=(-3, 0, 0))
bpy.data.objects['Cylinder'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Cylinder'].modifiers['Solidify'].solidify_mode = 'NON_MANIFOLD'
bpy.data.objects['Cylinder'].modifiers['Solidify'].thickness = 0.3
bpy.ops.mesh.primitive_monkey_add(location=(0, 4, 0))
bpy.data.objects['Suzanne'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Suzanne'].modifiers['Solidify'].solidify_mode = 'NON_MANIFOLD'
bpy.data.objects['Suzanne'].modifiers['Solidify'].thickness = 0.05
bpy.data.objects['Suzanne'].modifiers['Solidify'].offset = 0
