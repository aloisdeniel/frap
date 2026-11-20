# WP 6.10: SMOOTH weighted by `vertex_group`, inverted, with unassigned vertices (not moved), a
# locked axis and a missing group.
# golden: startup=factory compare=evaluated:A,evaluated:B,evaluated:C,evaluated:D tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1)
bpy.context.object.name = 'A'
bpy.data.objects['A'].vertex_groups.new(name='G')
bpy.data.objects['A'].vertex_groups['G'].add([6], 0.0, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([5, 11, 16, 21, 26, 31, 36, 41, 46, 51, 56, 61], 0.05, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([4, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55, 60], 0.25, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([3, 9, 14, 19, 24, 29, 34, 39, 44, 49, 54, 59], 0.5, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([2, 8, 13, 18, 23, 28, 33, 38, 43, 48, 53, 58], 0.75, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([1, 7, 12, 17, 22, 27, 32, 37, 42, 47, 52, 57], 0.95, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([0], 1.0, 'REPLACE')
bpy.data.objects['A'].modifiers.new(name='M', type='SMOOTH')
bpy.data.objects['A'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['A'].modifiers['M'].factor = 0.8
bpy.data.objects['A'].modifiers['M'].iterations = 3
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1)
bpy.context.object.name = 'B'
bpy.data.objects['B'].vertex_groups.new(name='G')
bpy.data.objects['B'].vertex_groups['G'].add([6], 0.0, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([5, 11, 16, 21, 26, 31, 36, 41, 46, 51, 56, 61], 0.05, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([4, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55, 60], 0.25, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([3, 9, 14, 19, 24, 29, 34, 39, 44, 49, 54, 59], 0.5, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([2, 8, 13, 18, 23, 28, 33, 38, 43, 48, 53, 58], 0.75, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([1, 7, 12, 17, 22, 27, 32, 37, 42, 47, 52, 57], 0.95, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([0], 1.0, 'REPLACE')
bpy.data.objects['B'].modifiers.new(name='M', type='SMOOTH')
bpy.data.objects['B'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['B'].modifiers['M'].factor = 0.8
bpy.data.objects['B'].modifiers['M'].iterations = 3
bpy.data.objects['B'].modifiers['M'].invert_vertex_group = True
bpy.ops.mesh.primitive_monkey_add(size=2)
bpy.context.object.name = 'C'
bpy.data.objects['C'].vertex_groups.new(name='G')
bpy.data.objects['C'].vertex_groups['G'].add([5, 10, 20, 25, 35, 40, 50, 55, 65, 70, 80, 85, 95, 100, 110, 115, 125, 130, 140, 145, 155, 160, 170, 175, 185, 190, 200, 205, 215, 220, 230, 235, 245, 250, 260, 265, 275, 280, 290, 295, 305, 310, 320, 325, 335, 340, 350, 355, 365, 370, 380, 385, 395, 400, 410, 415, 425, 430, 440, 445, 455, 460, 470, 475, 485, 490, 500, 505], 0.25, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([8, 13, 23, 28, 38, 43, 53, 58, 68, 73, 83, 88, 98, 103, 113, 118, 128, 133, 143, 148, 158, 163, 173, 178, 188, 193, 203, 208, 218, 223, 233, 238, 248, 253, 263, 268, 278, 283, 293, 298, 308, 313, 323, 328, 338, 343, 353, 358, 368, 373, 383, 388, 398, 403, 413, 418, 428, 433, 443, 448, 458, 463, 473, 478, 488, 493, 503], 0.4, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([1, 11, 16, 26, 31, 41, 46, 56, 61, 71, 76, 86, 91, 101, 106, 116, 121, 131, 136, 146, 151, 161, 166, 176, 181, 191, 196, 206, 211, 221, 226, 236, 241, 251, 256, 266, 271, 281, 286, 296, 301, 311, 316, 326, 331, 341, 346, 356, 361, 371, 376, 386, 391, 401, 406, 416, 421, 431, 436, 446, 451, 461, 466, 476, 481, 491, 496, 506], 0.5, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([4, 14, 19, 29, 34, 44, 49, 59, 64, 74, 79, 89, 94, 104, 109, 119, 124, 134, 139, 149, 154, 164, 169, 179, 184, 194, 199, 209, 214, 224, 229, 239, 244, 254, 259, 269, 274, 284, 289, 299, 304, 314, 319, 329, 334, 344, 349, 359, 364, 374, 379, 389, 394, 404, 409, 419, 424, 434, 439, 449, 454, 464, 469, 479, 484, 494, 499], 0.6, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([2, 7, 17, 22, 32, 37, 47, 52, 62, 67, 77, 82, 92, 97, 107, 112, 122, 127, 137, 142, 152, 157, 167, 172, 182, 187, 197, 202, 212, 217, 227, 232, 242, 247, 257, 262, 272, 277, 287, 292, 302, 307, 317, 322, 332, 337, 347, 352, 362, 367, 377, 382, 392, 397, 407, 412, 422, 427, 437, 442, 452, 457, 467, 472, 482, 487, 497, 502], 0.75, 'REPLACE')
bpy.data.objects['C'].modifiers.new(name='M', type='SMOOTH')
bpy.data.objects['C'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['C'].modifiers['M'].factor = 1.5
bpy.data.objects['C'].modifiers['M'].iterations = 3
bpy.data.objects['C'].modifiers['M'].use_y = False
bpy.ops.mesh.primitive_monkey_add(size=2)
bpy.context.object.name = 'D'
bpy.data.objects['D'].vertex_groups.new(name='H')
bpy.data.objects['D'].vertex_groups['H'].add([0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32, 34, 36, 38, 40, 42, 44, 46, 48, 50, 52, 54, 56, 58, 60, 62, 64, 78, 80, 82, 84, 86, 88, 91, 93, 95, 97, 99, 101, 103, 105, 107, 109, 111, 113, 115, 117, 119, 121, 123, 125, 127, 130, 132, 134, 139, 141, 143, 145, 147, 149, 151, 153, 155, 157, 159, 161, 163, 165, 167, 169, 171, 173, 178, 180, 182, 184, 186, 190, 192, 194, 196, 199, 201, 203, 205, 208, 210, 212, 214, 217, 219, 221, 224, 226, 228, 231, 233, 235, 237, 239, 241, 243, 245, 247, 249, 251, 253, 255, 257, 259, 261, 263, 265, 267, 269, 271, 273, 275, 277, 279, 281, 283, 285, 287, 289, 291, 293, 295, 297, 299, 309, 311, 313, 315, 317, 319, 321, 323, 325, 327, 329, 331, 333, 335, 337, 339, 341, 343, 345, 347, 349, 351, 353, 355, 357, 359, 361, 363, 365, 367, 369, 371, 373, 375, 377, 379, 381, 383, 385, 387, 389, 391, 393, 395, 397, 399, 401, 403, 405, 407, 409, 411, 413, 415, 417, 419, 421, 423, 425, 427, 429, 431, 433, 435, 437, 439, 441, 443, 445, 447, 449, 451, 453, 455, 457, 459, 461, 463, 465, 467, 469, 471, 473, 475, 477, 479, 481, 483, 485, 487, 489, 491, 493, 495, 497, 499, 501, 503, 505], 1.0, 'REPLACE')
bpy.data.objects['D'].modifiers.new(name='M', type='SMOOTH')
bpy.data.objects['D'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['D'].modifiers['M'].factor = 0.8
bpy.data.objects['D'].modifiers['M'].iterations = 3
