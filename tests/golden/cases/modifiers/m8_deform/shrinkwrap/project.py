# WP 8.3.12: SHRINKWRAP `PROJECT` (BVH watertight ray casts into a rotated, scaled Suzanne):
# along the vertex normals, along axes, negative and positive directions, back/front face
# culling (inverted for the negative rays), a projection limit, an auxiliary target, subdivided
# ray origins (`subsurf_levels`), `ABOVE_SURFACE` (smooth normals through the space transform) and
# `OUTSIDE_SURFACE`.
# golden: startup=empty compare=evaluated:A,evaluated:B,evaluated:C,evaluated:D,evaluated:E,evaluated:F,evaluated:G,evaluated:H,evaluated:I tier=T2
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.context.object.name = 'Target'
bpy.data.objects['Target'].rotation_euler = (0.3, 0.1, 0.2)
bpy.data.objects['Target'].scale = (1.2, 1, 0.9)
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=0.5, location=(0, -1.2, 0))
bpy.context.object.name = 'Aux'
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=0.3)
bpy.context.object.name = 'A'
bpy.data.objects['A'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['A'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['A'].modifiers['M'].wrap_method = 'PROJECT'
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=0.3)
bpy.context.object.name = 'B'
bpy.data.objects['B'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['B'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['B'].modifiers['M'].wrap_method = 'PROJECT'
bpy.data.objects['B'].modifiers['M'].use_project_z = True
bpy.data.objects['B'].modifiers['M'].use_negative_direction = True
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6, size=1.6, location=(0, -2, 0), rotation=(1.5707963267948966, 0, 0))
bpy.context.object.name = 'C'
bpy.data.objects['C'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['C'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['C'].modifiers['M'].wrap_method = 'PROJECT'
bpy.data.objects['C'].modifiers['M'].use_negative_direction = True
bpy.data.objects['C'].modifiers['M'].use_positive_direction = False
bpy.data.objects['C'].modifiers['M'].cull_face = 'BACK'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6, size=1.6, location=(0, -2, 0), rotation=(1.5707963267948966, 0, 0))
bpy.context.object.name = 'D'
bpy.data.objects['D'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['D'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['D'].modifiers['M'].wrap_method = 'PROJECT'
bpy.data.objects['D'].modifiers['M'].use_negative_direction = True
bpy.data.objects['D'].modifiers['M'].cull_face = 'FRONT'
bpy.data.objects['D'].modifiers['M'].use_invert_cull = True
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6, size=1.6, location=(-2, 0, 0), rotation=(0, 1.5707963267948966, 0))
bpy.context.object.name = 'E'
bpy.data.objects['E'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['E'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['E'].modifiers['M'].wrap_method = 'PROJECT'
bpy.data.objects['E'].modifiers['M'].use_project_x = True
bpy.data.objects['E'].modifiers['M'].use_negative_direction = True
bpy.data.objects['E'].modifiers['M'].project_limit = 1.6
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6, size=1.6, location=(0, -2.5, 0), rotation=(1.5707963267948966, 0, 0))
bpy.context.object.name = 'F'
bpy.data.objects['F'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['F'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['F'].modifiers['M'].auxiliary_target = bpy.data.objects['Aux']
bpy.data.objects['F'].modifiers['M'].wrap_method = 'PROJECT'
bpy.data.objects['F'].modifiers['M'].use_negative_direction = True
bpy.ops.mesh.primitive_cube_add(size=0.6)
bpy.context.object.name = 'G'
bpy.data.objects['G'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['G'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['G'].modifiers['M'].wrap_method = 'PROJECT'
bpy.data.objects['G'].modifiers['M'].subsurf_levels = 2
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=0.3)
bpy.context.object.name = 'H'
bpy.data.objects['H'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['H'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['H'].modifiers['M'].wrap_method = 'PROJECT'
bpy.data.objects['H'].modifiers['M'].wrap_mode = 'ABOVE_SURFACE'
bpy.data.objects['H'].modifiers['M'].offset = 0.1
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=0.3)
bpy.context.object.name = 'I'
bpy.data.objects['I'].modifiers.new(name='M', type='SHRINKWRAP')
bpy.data.objects['I'].modifiers['M'].target = bpy.data.objects['Target']
bpy.data.objects['I'].modifiers['M'].wrap_method = 'PROJECT'
bpy.data.objects['I'].modifiers['M'].wrap_mode = 'OUTSIDE_SURFACE'
bpy.data.objects['I'].modifiers['M'].offset = 0.05
