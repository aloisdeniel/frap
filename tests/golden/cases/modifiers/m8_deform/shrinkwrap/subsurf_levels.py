# WP 8.3.12: SHRINKWRAP `PROJECT` with `subsurf_levels` on an asymmetric mesh (a UV sphere
# projected onto a cube along its negative normals): Blender's non-adaptive Subdiv refines
# uniformly to the level, so the ray origins change with it and approach the limit surface.
# golden: startup=empty compare=evaluated:L1,evaluated:L2,evaluated:L3,evaluated:L6 tier=T2
import bpy
bpy.ops.mesh.primitive_cube_add(size=1.0)
bpy.context.object.name = 'Box'
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, radius=2)
bpy.context.object.name = 'L1'
bpy.data.objects['L1'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['L1'].modifiers['M'].target = bpy.data.objects['Box']
bpy.data.objects['L1'].modifiers['M'].wrap_method = 'PROJECT'
bpy.data.objects['L1'].modifiers['M'].subsurf_levels = 1
bpy.data.objects['L1'].modifiers['M'].use_negative_direction = True
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, radius=2)
bpy.context.object.name = 'L2'
bpy.data.objects['L2'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['L2'].modifiers['M'].target = bpy.data.objects['Box']
bpy.data.objects['L2'].modifiers['M'].wrap_method = 'PROJECT'
bpy.data.objects['L2'].modifiers['M'].subsurf_levels = 2
bpy.data.objects['L2'].modifiers['M'].use_negative_direction = True
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, radius=2)
bpy.context.object.name = 'L3'
bpy.data.objects['L3'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['L3'].modifiers['M'].target = bpy.data.objects['Box']
bpy.data.objects['L3'].modifiers['M'].wrap_method = 'PROJECT'
bpy.data.objects['L3'].modifiers['M'].subsurf_levels = 3
bpy.data.objects['L3'].modifiers['M'].use_negative_direction = True
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, radius=2)
bpy.context.object.name = 'L6'
bpy.data.objects['L6'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['L6'].modifiers['M'].target = bpy.data.objects['Box']
bpy.data.objects['L6'].modifiers['M'].wrap_method = 'PROJECT'
bpy.data.objects['L6'].modifiers['M'].subsurf_levels = 6
bpy.data.objects['L6'].modifiers['M'].use_negative_direction = True
