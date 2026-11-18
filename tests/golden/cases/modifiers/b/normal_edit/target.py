# WP 4.3: NORMAL_EDIT towards a target empty (DIRECTIONAL), all parallel
# (`use_direction_parallel`), and RADIAL about a scaled target; DIRECTIONAL without a target
# is disabled (the cube keeps its normals).
# golden: startup=factory compare=evaluated:Suzanne,evaluated:Sphere,evaluated:Cylinder,evaluated:Cube tier=T1
import bpy
bpy.ops.object.empty_add(location=(1, 0.5, 3))
bpy.data.objects['Empty'].scale = (1, 2, 0.5)
bpy.ops.mesh.primitive_monkey_add(location=(3, 0, 0))
bpy.ops.object.shade_smooth()
bpy.data.objects['Suzanne'].modifiers.new(name='M', type='NORMAL_EDIT')
bpy.data.objects['Suzanne'].modifiers['M'].mode = 'DIRECTIONAL'
bpy.data.objects['Suzanne'].modifiers['M'].target = bpy.data.objects['Empty']
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=8, location=(6, 0, 0))
bpy.ops.object.shade_smooth()
bpy.data.objects['Sphere'].modifiers.new(name='M', type='NORMAL_EDIT')
bpy.data.objects['Sphere'].modifiers['M'].mode = 'DIRECTIONAL'
bpy.data.objects['Sphere'].modifiers['M'].use_direction_parallel = True
bpy.data.objects['Sphere'].modifiers['M'].offset = (0.2, 0, 0)
bpy.data.objects['Sphere'].modifiers['M'].target = bpy.data.objects['Empty']
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(9, 0, 0))
bpy.ops.object.shade_smooth()
bpy.data.objects['Cylinder'].modifiers.new(name='M', type='NORMAL_EDIT')
bpy.data.objects['Cylinder'].modifiers['M'].target = bpy.data.objects['Empty']
bpy.data.objects['Cube'].modifiers.new(name='M', type='NORMAL_EDIT')
bpy.data.objects['Cube'].modifiers['M'].mode = 'DIRECTIONAL'
