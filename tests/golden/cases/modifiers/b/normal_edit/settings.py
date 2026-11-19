# WP 4.3: NORMAL_EDIT RADIAL from an offset centre (faces whose new normals point inwards are
# flipped), the same with `no_polynors_fix`, and ADD mixing limited to 0.3 rad on an ico sphere.
# golden: startup=empty compare=evaluated:Suzanne,evaluated:Suzanne.001,evaluated:Icosphere tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.shade_smooth()
bpy.data.objects['Suzanne'].modifiers.new(name='M', type='NORMAL_EDIT')
bpy.data.objects['Suzanne'].modifiers['M'].offset = (0, 3, 0)
bpy.ops.mesh.primitive_monkey_add(location=(3, 0, 0))
bpy.ops.object.shade_smooth()
bpy.data.objects['Suzanne.001'].modifiers.new(name='M', type='NORMAL_EDIT')
bpy.data.objects['Suzanne.001'].modifiers['M'].offset = (0, 3, 0)
bpy.data.objects['Suzanne.001'].modifiers['M'].no_polynors_fix = True
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, location=(6, 0, 0))
bpy.ops.object.shade_smooth()
bpy.data.objects['Icosphere'].modifiers.new(name='M', type='NORMAL_EDIT')
bpy.data.objects['Icosphere'].modifiers['M'].mix_mode = 'ADD'
bpy.data.objects['Icosphere'].modifiers['M'].mix_factor = 0.6
bpy.data.objects['Icosphere'].modifiers['M'].mix_limit = 0.3
bpy.data.objects['Icosphere'].modifiers['M'].offset = (0.5, 0, 0.2)
