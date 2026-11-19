# WP 4.2: ARRAY fit modes and caps: FIT_LENGTH makes as many copies as fit (7.0 / 2 + 1 = 4);
# FIT_CURVE without a curve uses the length too. Start and end caps (an ico sphere and a cone,
# their evaluated meshes) are placed one offset before the first and after the last copy, merged
# with them, and their UVs and materials copied where the attributes match.
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001 tier=T1
import bpy
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=1, location=(0, 5, 0))
bpy.ops.mesh.primitive_cone_add(vertices=8, radius1=1, depth=2, location=(0, -5, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.duplicate()
bpy.data.objects['Cube'].modifiers.new(name='Array', type='ARRAY')
bpy.data.objects['Cube'].modifiers['Array'].fit_type = 'FIT_LENGTH'
bpy.data.objects['Cube'].modifiers['Array'].fit_length = 7.0
bpy.data.objects['Cube'].modifiers['Array'].start_cap = bpy.data.objects['Icosphere']
bpy.data.objects['Cube'].modifiers['Array'].end_cap = bpy.data.objects['Cone']
bpy.data.objects['Cube'].modifiers['Array'].use_merge_vertices = True
bpy.data.objects['Cube.001'].modifiers.new(name='Array', type='ARRAY')
bpy.data.objects['Cube.001'].modifiers['Array'].fit_type = 'FIT_CURVE'
bpy.data.objects['Cube.001'].modifiers['Array'].fit_length = 3.5
bpy.data.objects['Cube.001'].modifiers['Array'].relative_offset_displace = (0, 0, 1)
bpy.data.objects['Cube.001'].modifiers['Array'].end_cap = bpy.data.objects['Cone']
