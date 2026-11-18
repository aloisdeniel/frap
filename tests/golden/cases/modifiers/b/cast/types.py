# WP 4.3: CAST CYLINDER limited by a radius (Z untouched), and CUBOID with a fixed size and
# without the Y axis.
# golden: startup=empty compare=evaluated:Suzanne,evaluated:Sphere tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.data.objects['Suzanne'].modifiers.new(name='M', type='CAST')
bpy.data.objects['Suzanne'].modifiers['M'].cast_type = 'CYLINDER'
bpy.data.objects['Suzanne'].modifiers['M'].radius = 0.8
bpy.data.objects['Suzanne'].modifiers['M'].factor = 1.0
bpy.ops.mesh.primitive_uv_sphere_add(segments=16, ring_count=8, location=(3, 0, 0))
bpy.data.objects['Sphere'].modifiers.new(name='M', type='CAST')
bpy.data.objects['Sphere'].modifiers['M'].cast_type = 'CUBOID'
bpy.data.objects['Sphere'].modifiers['M'].use_radius_as_size = False
bpy.data.objects['Sphere'].modifiers['M'].size = 0.7
bpy.data.objects['Sphere'].modifiers['M'].factor = 0.8
bpy.data.objects['Sphere'].modifiers['M'].use_y = False
