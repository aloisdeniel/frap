# WP 4.2: SOLIDIFY "Simple" offsets: even thickness with high-quality normals on Suzanne (open
# eyes: rims), the thickness clamp (shortest edge) on a UV sphere, and the angle clamp with even
# thickness on a cone.
# golden: startup=empty compare=evaluated:Suzanne,evaluated:Sphere,evaluated:Cone tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.data.objects['Suzanne'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Suzanne'].modifiers['Solidify'].thickness = 0.05
bpy.data.objects['Suzanne'].modifiers['Solidify'].use_even_offset = True
bpy.data.objects['Suzanne'].modifiers['Solidify'].use_quality_normals = True
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, location=(3, 0, 0))
bpy.data.objects['Sphere'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Sphere'].modifiers['Solidify'].thickness = 0.8
bpy.data.objects['Sphere'].modifiers['Solidify'].thickness_clamp = 0.5
bpy.ops.mesh.primitive_cone_add(vertices=6, location=(-3, 0, 0))
bpy.data.objects['Cone'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Cone'].modifiers['Solidify'].thickness = 0.6
bpy.data.objects['Cone'].modifiers['Solidify'].use_even_offset = True
bpy.data.objects['Cone'].modifiers['Solidify'].thickness_clamp = 0.7
bpy.data.objects['Cone'].modifiers['Solidify'].use_thickness_angle_clamp = True
