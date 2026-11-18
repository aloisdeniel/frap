# WP 4.3: SMOOTH factor 1.5 (over-relaxation), 4 iterations, Y locked, on a UV sphere; a
# factor of 0 disables the modifier (the second sphere keeps its shape). UV-sphere topology
# is compared canonically (D-19).
# golden: startup=empty compare=evaluated:Sphere,evaluated:Sphere.001 tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=8)
bpy.data.objects['Sphere'].modifiers.new(name='M', type='SMOOTH')
bpy.data.objects['Sphere'].modifiers['M'].factor = 1.5
bpy.data.objects['Sphere'].modifiers['M'].iterations = 4
bpy.data.objects['Sphere'].modifiers['M'].use_y = False
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=8, location=(3, 0, 0))
bpy.data.objects['Sphere.001'].modifiers.new(name='M', type='SMOOTH')
bpy.data.objects['Sphere.001'].modifiers['M'].factor = 0.0
