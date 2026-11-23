# WP 8.3.12: SHRINKWRAP `NEAREST_VERTEX` (vertex BVH, the offset kept along the line to the
# nearest vertex) and `TARGET_PROJECT` (the Newton solve for the point whose interpolated
# normal passes through the vertex, BVH optimal-order search) on Suzanne and on a bumpy open
# grid (the boundary edges as thin cylinders), in several wrap modes.
# golden: startup=empty compare=evaluated:A,evaluated:B,evaluated:C,evaluated:D,evaluated:E,evaluated:F,evaluated:G tier=T2
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.context.object.name = 'Target'
bpy.data.objects['Target'].rotation_euler = (0.3, 0.1, 0.2)
bpy.data.objects['Target'].scale = (1.2, 1, 0.9)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=8, y_subdivisions=8, size=2, location=(0, 0, -3))
bpy.context.object.name = 'Grid'
bpy.data.objects['Grid'].modifiers.new(name='Bump', type='WAVE')
bpy.data.objects['Grid'].modifiers['Bump'].height = 0.4
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1.4)
bpy.context.object.name = 'A'
bpy.data.objects['A'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['A'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['A'].modifiers['M'].wrap_method = 'NEAREST_VERTEX'
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1.4)
bpy.context.object.name = 'B'
bpy.data.objects['B'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['B'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['B'].modifiers['M'].wrap_method = 'NEAREST_VERTEX'
bpy.data.objects['B'].modifiers['M'].offset = 0.15
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1.4)
bpy.context.object.name = 'C'
bpy.data.objects['C'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['C'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['C'].modifiers['M'].wrap_method = 'TARGET_PROJECT'
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1.4)
bpy.context.object.name = 'D'
bpy.data.objects['D'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['D'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['D'].modifiers['M'].wrap_method = 'TARGET_PROJECT'
bpy.data.objects['D'].modifiers['M'].wrap_mode = 'OUTSIDE'
bpy.data.objects['D'].modifiers['M'].offset = 0.1
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1.4)
bpy.context.object.name = 'E'
bpy.data.objects['E'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['E'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['E'].modifiers['M'].wrap_method = 'TARGET_PROJECT'
bpy.data.objects['E'].modifiers['M'].wrap_mode = 'ABOVE_SURFACE'
bpy.data.objects['E'].modifiers['M'].offset = 0.1
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1.6, location=(0.3, 0.2, -2.6))
bpy.context.object.name = 'F'
bpy.data.objects['F'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['F'].modifiers['M'].target = bpy.data.objects['Grid']
bpy.data.objects['F'].modifiers['M'].wrap_method = 'TARGET_PROJECT'
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1.6, location=(0.3, 0.2, -2.6))
bpy.context.object.name = 'G'
bpy.data.objects['G'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['G'].modifiers['M'].target = bpy.data.objects['Grid']
bpy.data.objects['G'].modifiers['M'].wrap_method = 'NEAREST_SURFACEPOINT'
bpy.data.objects['G'].modifiers['M'].wrap_mode = 'ABOVE_SURFACE'
bpy.data.objects['G'].modifiers['M'].offset = 0.2
