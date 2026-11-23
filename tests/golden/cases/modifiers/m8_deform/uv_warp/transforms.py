# WP 8.3.12: UV_WARP: the direct 2D transform (offset, scale, rotation about the centre), the
# warp between two empties on chosen axes, a vertex group (inverted), a named UV map (and a
# missing name, which falls back to the active map) and a mesh without UV maps.
# golden: startup=empty compare=evaluated:A,evaluated:B,evaluated:C,evaluated:D,evaluated:E,evaluated:F tier=T2
import bpy
bpy.ops.object.empty_add(location=(0.2, 0.1, 0))
bpy.context.object.name = 'From'
bpy.ops.object.empty_add(location=(0.5, -0.3, 0.2), rotation=(0.1, 0.2, 0.6))
bpy.context.object.name = 'To'
bpy.data.objects['To'].scale = (1.3, 0.8, 1)
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6)
bpy.context.object.name = 'A'
bpy.data.objects['A'].modifiers.new(name='M', type='UV_WARP')
bpy.data.objects['A'].modifiers['M'].offset = (0.1, -0.2)
bpy.data.objects['A'].modifiers['M'].scale = (1.5, 0.7)
bpy.data.objects['A'].modifiers['M'].rotation = 0.6
bpy.data.objects['A'].modifiers['M'].center = (0.3, 0.6)
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6)
bpy.context.object.name = 'B'
bpy.data.objects['B'].modifiers.new(name='M', type='UV_WARP')
bpy.data.objects['B'].modifiers['M'].object_from = bpy.data.objects['From']
bpy.data.objects['B'].modifiers['M'].object_to = bpy.data.objects['To']
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6)
bpy.context.object.name = 'C'
bpy.data.objects['C'].modifiers.new(name='M', type='UV_WARP')
bpy.data.objects['C'].modifiers['M'].object_from = bpy.data.objects['From']
bpy.data.objects['C'].modifiers['M'].object_to = bpy.data.objects['To']
bpy.data.objects['C'].modifiers['M'].axis_u = 'Y'
bpy.data.objects['C'].modifiers['M'].axis_v = 'Z'
bpy.data.objects['C'].modifiers['M'].center = (0, 0)
bpy.data.objects['C'].modifiers['M'].rotation = -0.3
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6)
bpy.context.object.name = 'D'
bpy.data.objects['D'].vertex_groups.new(name='G')
bpy.data.objects['D'].vertex_groups['G'].add([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29], 1.0, 'REPLACE')
bpy.data.objects['D'].vertex_groups['G'].add([30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49], 0.5, 'REPLACE')
bpy.data.objects['D'].modifiers.new(name='M', type='UV_WARP')
bpy.data.objects['D'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['D'].modifiers['M'].invert_vertex_group = True
bpy.data.objects['D'].modifiers['M'].offset = (0.5, 0.25)
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6)
bpy.context.object.name = 'E'
bpy.data.meshes['Sphere.004'].uv_layers.new(name='Second')
bpy.data.objects['E'].modifiers.new(name='M', type='UV_WARP')
bpy.data.objects['E'].modifiers['M'].uv_layer = 'Second'
bpy.data.objects['E'].modifiers['M'].scale = (2, 2)
bpy.data.objects['E'].modifiers.new(name='M2', type='UV_WARP')
bpy.data.objects['E'].modifiers['M2'].uv_layer = 'Missing'
bpy.data.objects['E'].modifiers['M2'].offset = (0.1, 0.1)
bpy.data.objects['E'].modifiers['M2'].uv_layer
bpy.ops.mesh.primitive_cube_add()
bpy.context.object.name = 'F'
bpy.data.meshes['Cube'].uv_layers.remove(bpy.data.meshes['Cube'].uv_layers['UVMap'])
bpy.data.objects['F'].modifiers.new(name='M', type='UV_WARP')
bpy.data.objects['F'].modifiers['M'].offset = (0.5, 0.5)
