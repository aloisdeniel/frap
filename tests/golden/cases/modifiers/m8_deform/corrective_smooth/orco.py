# WP 8.3.12: CORRECTIVE_SMOOTH with `rest_source='ORCO'` after a SIMPLE_DEFORM twist and a
# DISPLACE: SIMPLE and LENGTH_WEIGHTED smoothing, scale, iterations and factor, pinned
# boundaries on an open grid, `use_only_smooth`, a vertex group (inverted), and `BIND` without
# bind data (Blender's "Bind data required": the mesh passes through).
# golden: startup=empty compare=evaluated:A,evaluated:B,evaluated:C,evaluated:D,evaluated:E,evaluated:F,evaluated:G,evaluated:H tier=T2
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.context.object.name = 'A'
bpy.data.objects['A'].modifiers.new(name='Twist', type='SIMPLE_DEFORM')
bpy.data.objects['A'].modifiers['Twist'].angle = 0.8
bpy.data.objects['A'].modifiers.new(name='M', type='CORRECTIVE_SMOOTH')
bpy.ops.mesh.primitive_monkey_add()
bpy.context.object.name = 'B'
bpy.data.objects['B'].modifiers.new(name='Twist', type='SIMPLE_DEFORM')
bpy.data.objects['B'].modifiers['Twist'].angle = 0.8
bpy.data.objects['B'].modifiers.new(name='M', type='CORRECTIVE_SMOOTH')
bpy.data.objects['B'].modifiers['M'].smooth_type = 'LENGTH_WEIGHTED'
bpy.data.objects['B'].modifiers['M'].iterations = 10
bpy.data.objects['B'].modifiers['M'].scale = 0.8
bpy.ops.mesh.primitive_monkey_add()
bpy.context.object.name = 'C'
bpy.data.objects['C'].modifiers.new(name='Push', type='DISPLACE')
bpy.data.objects['C'].modifiers['Push'].strength = 0.3
bpy.data.objects['C'].modifiers.new(name='M', type='CORRECTIVE_SMOOTH')
bpy.data.objects['C'].modifiers['M'].factor = 0.7
bpy.data.objects['C'].modifiers['M'].iterations = 3
bpy.ops.mesh.primitive_grid_add(x_subdivisions=8, y_subdivisions=8, size=2)
bpy.context.object.name = 'D'
bpy.data.objects['D'].modifiers.new(name='Bend', type='SIMPLE_DEFORM')
bpy.data.objects['D'].modifiers['Bend'].deform_method = 'BEND'
bpy.data.objects['D'].modifiers['Bend'].angle = 1.2
bpy.data.objects['D'].modifiers.new(name='M', type='CORRECTIVE_SMOOTH')
bpy.data.objects['D'].modifiers['M'].use_pin_boundary = True
bpy.ops.mesh.primitive_monkey_add()
bpy.context.object.name = 'E'
bpy.data.objects['E'].modifiers.new(name='M', type='CORRECTIVE_SMOOTH')
bpy.data.objects['E'].modifiers['M'].use_only_smooth = True
bpy.data.objects['E'].modifiers['M'].iterations = 4
bpy.ops.mesh.primitive_monkey_add()
bpy.context.object.name = 'F'
bpy.data.objects['F'].vertex_groups.new(name='G')
bpy.data.objects['F'].vertex_groups['G'].add([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199], 1.0, 'REPLACE')
bpy.data.objects['F'].vertex_groups['G'].add([200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254, 255, 256, 257, 258, 259, 260, 261, 262, 263, 264, 265, 266, 267, 268, 269, 270, 271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 281, 282, 283, 284, 285, 286, 287, 288, 289, 290, 291, 292, 293, 294, 295, 296, 297, 298, 299], 0.4, 'REPLACE')
bpy.data.objects['F'].modifiers.new(name='Twist', type='SIMPLE_DEFORM')
bpy.data.objects['F'].modifiers['Twist'].angle = 0.8
bpy.data.objects['F'].modifiers.new(name='M', type='CORRECTIVE_SMOOTH')
bpy.data.objects['F'].modifiers['M'].vertex_group = 'G'
bpy.ops.mesh.primitive_monkey_add()
bpy.context.object.name = 'G'
bpy.data.objects['G'].vertex_groups.new(name='G')
bpy.data.objects['G'].vertex_groups['G'].add([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199], 0.6, 'REPLACE')
bpy.data.objects['G'].modifiers.new(name='Twist', type='SIMPLE_DEFORM')
bpy.data.objects['G'].modifiers['Twist'].angle = 0.8
bpy.data.objects['G'].modifiers.new(name='M', type='CORRECTIVE_SMOOTH')
bpy.data.objects['G'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['G'].modifiers['M'].invert_vertex_group = True
bpy.data.objects['G'].modifiers['M'].smooth_type = 'LENGTH_WEIGHTED'
bpy.data.objects['G'].modifiers['M'].use_pin_boundary = True
bpy.ops.mesh.primitive_monkey_add()
bpy.context.object.name = 'H'
bpy.data.objects['H'].modifiers.new(name='Twist', type='SIMPLE_DEFORM')
bpy.data.objects['H'].modifiers['Twist'].angle = 0.8
bpy.data.objects['H'].modifiers.new(name='M', type='CORRECTIVE_SMOOTH')
bpy.data.objects['H'].modifiers['M'].rest_source = 'BIND'
