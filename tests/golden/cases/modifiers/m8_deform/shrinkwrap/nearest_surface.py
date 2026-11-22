# WP 8.3.12: SHRINKWRAP `NEAREST_SURFACEPOINT` (BVH nearest point on the corner triangles of a
# rotated, scaled Suzanne) in every `wrap_mode`, with an offset, a vertex group (partial
# weights, inverted) and a missing group (full weight).
# golden: startup=empty compare=evaluated:A,evaluated:B,evaluated:C,evaluated:D,evaluated:E,evaluated:F,evaluated:G,evaluated:H tier=T2
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.context.object.name = 'Target'
bpy.data.objects['Target'].rotation_euler = (0.3, 0.1, 0.2)
bpy.data.objects['Target'].scale = (1.2, 1, 0.9)
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1.4)
bpy.context.object.name = 'A'
bpy.data.objects['A'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['A'].modifiers['M'].target = bpy.data.objects['Target']
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1.4)
bpy.context.object.name = 'B'
bpy.data.objects['B'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['B'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['B'].modifiers['M'].wrap_mode = 'INSIDE'
bpy.data.objects['B'].modifiers['M'].offset = 0.1
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1.4)
bpy.context.object.name = 'C'
bpy.data.objects['C'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['C'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['C'].modifiers['M'].wrap_mode = 'OUTSIDE'
bpy.data.objects['C'].modifiers['M'].offset = 0.1
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=0.6)
bpy.context.object.name = 'D'
bpy.data.objects['D'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['D'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['D'].modifiers['M'].wrap_mode = 'OUTSIDE_SURFACE'
bpy.data.objects['D'].modifiers['M'].offset = 0.05
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1.4)
bpy.context.object.name = 'E'
bpy.data.objects['E'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['E'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['E'].modifiers['M'].wrap_mode = 'ABOVE_SURFACE'
bpy.data.objects['E'].modifiers['M'].offset = 0.1
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1.4, location=(0.2, -0.1, 0.3))
bpy.context.object.name = 'F'
bpy.data.objects['F'].vertex_groups.new(name='G')
bpy.data.objects['F'].vertex_groups['G'].add([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], 1.0, 'REPLACE')
bpy.data.objects['F'].vertex_groups['G'].add([12, 13, 14, 15, 16, 17, 18, 19], 0.5, 'REPLACE')
bpy.data.objects['F'].vertex_groups['G'].add([20, 21, 22, 23], 0.25, 'REPLACE')
bpy.data.objects['F'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['F'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['F'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['F'].modifiers['M'].offset = -0.05
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1.4, location=(0.2, -0.1, 0.3))
bpy.context.object.name = 'G'
bpy.data.objects['G'].vertex_groups.new(name='G')
bpy.data.objects['G'].vertex_groups['G'].add([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], 1.0, 'REPLACE')
bpy.data.objects['G'].vertex_groups['G'].add([12, 13, 14, 15, 16, 17, 18, 19], 0.5, 'REPLACE')
bpy.data.objects['G'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['G'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['G'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['G'].modifiers['M'].invert_vertex_group = True
bpy.data.objects['G'].modifiers['M'].wrap_mode = 'OUTSIDE'
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1.4, rotation=(0.5, 0, 0.4))
bpy.context.object.name = 'H'
bpy.data.objects['H'].scale = (1, 1.3, 1)
bpy.data.objects['H'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['H'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['H'].modifiers['M'].vertex_group = 'Missing'
bpy.data.objects['H'].modifiers['M'].wrap_mode = 'ON_SURFACE'
bpy.data.objects['H'].modifiers['M'].offset = 0.2
