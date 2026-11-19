# WP 4.3: SIMPLE_DEFORM TAPER along Y with X locked (factor), and STRETCH along Z with a
# negative factor and Y locked.
# golden: startup=empty compare=evaluated:Suzanne,evaluated:Sphere tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.data.objects['Suzanne'].modifiers.new(name='M', type='SIMPLE_DEFORM')
bpy.data.objects['Suzanne'].modifiers['M'].deform_method = 'TAPER'
bpy.data.objects['Suzanne'].modifiers['M'].deform_axis = 'Y'
bpy.data.objects['Suzanne'].modifiers['M'].factor = 0.5
bpy.data.objects['Suzanne'].modifiers['M'].lock_x = True
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=8, location=(3, 0, 0))
bpy.data.objects['Sphere'].modifiers.new(name='M', type='SIMPLE_DEFORM')
bpy.data.objects['Sphere'].modifiers['M'].deform_method = 'STRETCH'
bpy.data.objects['Sphere'].modifiers['M'].deform_axis = 'Z'
bpy.data.objects['Sphere'].modifiers['M'].factor = -0.3
bpy.data.objects['Sphere'].modifiers['M'].lock_y = True
bpy.data.objects['Sphere'].modifiers['M'].limits = (0.2, 1.0)
