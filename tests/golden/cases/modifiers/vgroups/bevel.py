# WP 6.10: BEVEL `limit_method='VGROUP'`: edges whose two vertices weigh at least 0.5 (EDGES) or
# those vertices (VERTICES), inverted, with partial weights, and with a missing group (every edge is
# bevelled) and a group without weights (none is).
# golden: startup=factory compare=evaluated:A,evaluated:B,evaluated:C,evaluated:D,evaluated:E,evaluated:F,evaluated:G tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(size=2)
bpy.context.object.name = 'A'
bpy.data.objects['A'].vertex_groups.new(name='G')
bpy.data.objects['A'].vertex_groups['G'].add([1, 3, 5, 7], 1.0, 'REPLACE')
bpy.data.objects['A'].modifiers.new(name='M', type='BEVEL')
bpy.data.objects['A'].modifiers['M'].limit_method = 'VGROUP'
bpy.data.objects['A'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['A'].modifiers['M'].width = 0.2
bpy.ops.mesh.primitive_cube_add(size=2)
bpy.context.object.name = 'B'
bpy.data.objects['B'].vertex_groups.new(name='G')
bpy.data.objects['B'].vertex_groups['G'].add([1, 3, 5, 7], 1.0, 'REPLACE')
bpy.data.objects['B'].modifiers.new(name='M', type='BEVEL')
bpy.data.objects['B'].modifiers['M'].limit_method = 'VGROUP'
bpy.data.objects['B'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['B'].modifiers['M'].width = 0.2
bpy.data.objects['B'].modifiers['M'].invert_vertex_group = True
bpy.ops.mesh.primitive_cube_add(size=2)
bpy.context.object.name = 'C'
bpy.data.objects['C'].vertex_groups.new(name='G')
bpy.data.objects['C'].vertex_groups['G'].add([6, 7], 1.0, 'REPLACE')
bpy.data.objects['C'].modifiers.new(name='M', type='BEVEL')
bpy.data.objects['C'].modifiers['M'].limit_method = 'VGROUP'
bpy.data.objects['C'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['C'].modifiers['M'].width = 0.2
bpy.data.objects['C'].modifiers['M'].affect = 'VERTICES'
bpy.ops.mesh.primitive_cube_add(size=2)
bpy.context.object.name = 'D'
bpy.data.objects['D'].vertex_groups.new(name='G')
bpy.data.objects['D'].vertex_groups['G'].add([0, 1, 2, 3], 0.0, 'REPLACE')
bpy.data.objects['D'].vertex_groups['G'].add([4, 5, 6, 7], 1.0, 'REPLACE')
bpy.data.objects['D'].modifiers.new(name='M', type='BEVEL')
bpy.data.objects['D'].modifiers['M'].limit_method = 'VGROUP'
bpy.data.objects['D'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['D'].modifiers['M'].width = 0.2
bpy.data.objects['D'].modifiers['M'].segments = 2
bpy.ops.mesh.primitive_cube_add(size=2)
bpy.context.object.name = 'E'
bpy.data.objects['E'].vertex_groups.new(name='H')
bpy.data.objects['E'].vertex_groups['H'].add([1, 3, 5, 7], 1.0, 'REPLACE')
bpy.data.objects['E'].modifiers.new(name='M', type='BEVEL')
bpy.data.objects['E'].modifiers['M'].limit_method = 'VGROUP'
bpy.data.objects['E'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['E'].modifiers['M'].width = 0.2
bpy.ops.mesh.primitive_cube_add(size=2)
bpy.context.object.name = 'F'
bpy.data.objects['F'].vertex_groups.new(name='G')
bpy.data.objects['F'].vertex_groups.new(name='H')
bpy.data.objects['F'].vertex_groups['H'].add([1, 3, 5, 7], 1.0, 'REPLACE')
bpy.data.objects['F'].modifiers.new(name='M', type='BEVEL')
bpy.data.objects['F'].modifiers['M'].limit_method = 'VGROUP'
bpy.data.objects['F'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['F'].modifiers['M'].width = 0.2
bpy.ops.mesh.primitive_monkey_add(size=2)
bpy.context.object.name = 'G'
bpy.data.objects['G'].vertex_groups.new(name='G')
bpy.data.objects['G'].vertex_groups['G'].add([0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32, 34, 36, 38, 40, 42, 44, 46, 48, 50, 52, 54, 56, 58, 60, 62, 64, 78, 80, 82, 84, 86, 88, 91, 93, 95, 97, 99, 101, 103, 105, 107, 109, 111, 113, 115, 117, 119, 121, 123, 125, 127, 130, 132, 134, 139, 141, 143, 145, 147, 149, 151, 153, 155, 157, 159, 161, 163, 165, 167, 169, 171, 173, 178, 180, 182, 184, 186, 190, 192, 194, 196, 199, 201, 203, 205, 208, 210, 212, 214, 217, 219, 221, 224, 226, 228, 231, 233, 235, 237, 239, 241, 243, 245, 247, 249, 251, 253, 255, 257, 259, 261, 263, 265, 267, 269, 271, 273, 275, 277, 279, 281, 283, 285, 287, 289, 291, 293, 295, 297, 299, 309, 311, 313, 315, 317, 319, 321, 323, 325, 327, 329, 331, 333, 335, 337, 339, 341, 343, 345, 347, 349, 351, 353, 355, 357, 359, 361, 363, 365, 367, 369, 371, 373, 375, 377, 379, 381, 383, 385, 387, 389, 391, 393, 395, 397, 399, 401, 403, 405, 407, 409, 411, 413, 415, 417, 419, 421, 423, 425, 427, 429, 431, 433, 435, 437, 439, 441, 443, 445, 447, 449, 451, 453, 455, 457, 459, 461, 463, 465, 467, 469, 471, 473, 475, 477, 479, 481, 483, 485, 487, 489, 491, 493, 495, 497, 499, 501, 503, 505], 1.0, 'REPLACE')
bpy.data.objects['G'].modifiers.new(name='M', type='BEVEL')
bpy.data.objects['G'].modifiers['M'].limit_method = 'VGROUP'
bpy.data.objects['G'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['G'].modifiers['M'].width = 0.05
