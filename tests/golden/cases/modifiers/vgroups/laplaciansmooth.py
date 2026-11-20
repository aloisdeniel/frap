# WP 6.10: LAPLACIANSMOOTH weighted by `vertex_group`, inverted, with unassigned vertices,
# without volume preservation and with a missing group.
# golden: startup=factory compare=evaluated:A,evaluated:B,evaluated:C,evaluated:D,evaluated:E tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_monkey_add(size=2)
bpy.context.object.name = 'A'
bpy.data.objects['A'].vertex_groups.new(name='G')
bpy.data.objects['A'].vertex_groups['G'].add([392, 394, 396, 398, 400, 402, 404, 406, 408, 410, 420, 422, 424, 426, 428, 464, 466, 468, 470, 472, 476, 482, 484, 486, 488, 490, 492, 494, 496, 498, 500, 502], 0.0, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([98, 310, 312, 388, 414, 416, 418, 450, 452, 456, 474, 478, 480, 506], 0.05, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([96, 294, 296, 314, 368, 370, 376, 382, 390, 412, 430, 432, 434, 436, 438, 440, 448, 454, 458, 460, 462, 504], 0.1, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([100, 150, 152, 154, 292, 318, 362, 442, 444, 446], 0.15, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([43, 45, 94, 120, 122, 124, 148, 242, 244, 246, 270, 298, 320, 322, 364, 366, 372, 374, 378], 0.2, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([3, 5, 39, 41, 47, 49, 102, 118, 248, 266, 268, 272, 290, 316, 354, 356, 358, 360, 380], 0.25, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([1, 7, 9, 11, 31, 33, 35, 37, 51, 53, 61, 63, 65, 83, 85, 92, 116, 126, 146, 156, 240, 250, 264, 274, 300, 384, 386], 0.3, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([13, 25, 55, 59, 81, 87, 104, 158, 170, 179, 209, 236, 238, 276, 278, 288, 324, 330, 334, 350, 352], 0.35, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([15, 17, 23, 27, 29, 57, 79, 89, 106, 110, 114, 128, 160, 166, 168, 172, 174, 181, 211, 213, 215, 232, 234, 252, 254, 256, 258, 260, 262, 280, 282, 286, 326, 328, 332, 340, 342, 344, 346, 348], 0.4, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([19, 21, 108, 112, 131, 133, 135, 140, 142, 144, 162, 164, 183, 185, 187, 191, 193, 195, 202, 204, 206, 220, 222, 225, 227, 284, 336, 338], 0.45, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 90, 129, 136, 137, 138, 175, 176, 177, 188, 189, 196, 197, 198, 199, 200, 207, 216, 217, 218, 223, 228, 229, 230, 301, 302, 303, 304, 305, 306, 307, 308], 0.5, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([18, 20, 107, 111, 130, 132, 134, 139, 141, 143, 161, 163, 182, 184, 186, 190, 192, 194, 201, 203, 205, 219, 221, 224, 226, 283, 335, 337], 0.55, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([14, 16, 22, 26, 28, 56, 78, 88, 105, 109, 113, 127, 159, 165, 167, 171, 173, 180, 210, 212, 214, 231, 233, 251, 253, 255, 257, 259, 261, 279, 281, 285, 325, 327, 331, 339, 341, 343, 345, 347], 0.6, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([12, 24, 54, 58, 80, 86, 103, 157, 169, 178, 208, 235, 237, 275, 277, 287, 323, 329, 333, 349, 351], 0.65, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([0, 6, 8, 10, 30, 32, 34, 36, 50, 52, 60, 62, 64, 82, 84, 91, 115, 125, 145, 155, 239, 249, 263, 273, 299, 383, 385], 0.7, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([2, 4, 38, 40, 46, 48, 101, 117, 247, 265, 267, 271, 289, 315, 353, 355, 357, 359, 379], 0.75, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([42, 44, 93, 119, 121, 123, 147, 241, 243, 245, 269, 297, 319, 321, 363, 365, 371, 373, 377], 0.8, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([99, 149, 151, 153, 291, 317, 361, 441, 443, 445], 0.85, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([95, 293, 295, 313, 367, 369, 375, 381, 389, 411, 429, 431, 433, 435, 437, 439, 447, 453, 457, 459, 461, 503], 0.9, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([97, 309, 311, 387, 413, 415, 417, 449, 451, 455, 473, 477, 479, 505], 0.95, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([391, 393, 395, 397, 399, 401, 403, 405, 407, 409, 419, 421, 423, 425, 427, 463, 465, 467, 469, 471, 475, 481, 483, 485, 487, 489, 491, 493, 495, 497, 499, 501], 1.0, 'REPLACE')
bpy.data.objects['A'].modifiers.new(name='M', type='LAPLACIANSMOOTH')
bpy.data.objects['A'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['A'].modifiers['M'].lambda_factor = 2.0
bpy.data.objects['A'].modifiers['M'].iterations = 3
bpy.ops.mesh.primitive_monkey_add(size=2)
bpy.context.object.name = 'B'
bpy.data.objects['B'].vertex_groups.new(name='G')
bpy.data.objects['B'].vertex_groups['G'].add([392, 394, 396, 398, 400, 402, 404, 406, 408, 410, 420, 422, 424, 426, 428, 464, 466, 468, 470, 472, 476, 482, 484, 486, 488, 490, 492, 494, 496, 498, 500, 502], 0.0, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([98, 310, 312, 388, 414, 416, 418, 450, 452, 456, 474, 478, 480, 506], 0.05, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([96, 294, 296, 314, 368, 370, 376, 382, 390, 412, 430, 432, 434, 436, 438, 440, 448, 454, 458, 460, 462, 504], 0.1, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([100, 150, 152, 154, 292, 318, 362, 442, 444, 446], 0.15, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([43, 45, 94, 120, 122, 124, 148, 242, 244, 246, 270, 298, 320, 322, 364, 366, 372, 374, 378], 0.2, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([3, 5, 39, 41, 47, 49, 102, 118, 248, 266, 268, 272, 290, 316, 354, 356, 358, 360, 380], 0.25, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([1, 7, 9, 11, 31, 33, 35, 37, 51, 53, 61, 63, 65, 83, 85, 92, 116, 126, 146, 156, 240, 250, 264, 274, 300, 384, 386], 0.3, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([13, 25, 55, 59, 81, 87, 104, 158, 170, 179, 209, 236, 238, 276, 278, 288, 324, 330, 334, 350, 352], 0.35, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([15, 17, 23, 27, 29, 57, 79, 89, 106, 110, 114, 128, 160, 166, 168, 172, 174, 181, 211, 213, 215, 232, 234, 252, 254, 256, 258, 260, 262, 280, 282, 286, 326, 328, 332, 340, 342, 344, 346, 348], 0.4, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([19, 21, 108, 112, 131, 133, 135, 140, 142, 144, 162, 164, 183, 185, 187, 191, 193, 195, 202, 204, 206, 220, 222, 225, 227, 284, 336, 338], 0.45, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 90, 129, 136, 137, 138, 175, 176, 177, 188, 189, 196, 197, 198, 199, 200, 207, 216, 217, 218, 223, 228, 229, 230, 301, 302, 303, 304, 305, 306, 307, 308], 0.5, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([18, 20, 107, 111, 130, 132, 134, 139, 141, 143, 161, 163, 182, 184, 186, 190, 192, 194, 201, 203, 205, 219, 221, 224, 226, 283, 335, 337], 0.55, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([14, 16, 22, 26, 28, 56, 78, 88, 105, 109, 113, 127, 159, 165, 167, 171, 173, 180, 210, 212, 214, 231, 233, 251, 253, 255, 257, 259, 261, 279, 281, 285, 325, 327, 331, 339, 341, 343, 345, 347], 0.6, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([12, 24, 54, 58, 80, 86, 103, 157, 169, 178, 208, 235, 237, 275, 277, 287, 323, 329, 333, 349, 351], 0.65, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([0, 6, 8, 10, 30, 32, 34, 36, 50, 52, 60, 62, 64, 82, 84, 91, 115, 125, 145, 155, 239, 249, 263, 273, 299, 383, 385], 0.7, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([2, 4, 38, 40, 46, 48, 101, 117, 247, 265, 267, 271, 289, 315, 353, 355, 357, 359, 379], 0.75, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([42, 44, 93, 119, 121, 123, 147, 241, 243, 245, 269, 297, 319, 321, 363, 365, 371, 373, 377], 0.8, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([99, 149, 151, 153, 291, 317, 361, 441, 443, 445], 0.85, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([95, 293, 295, 313, 367, 369, 375, 381, 389, 411, 429, 431, 433, 435, 437, 439, 447, 453, 457, 459, 461, 503], 0.9, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([97, 309, 311, 387, 413, 415, 417, 449, 451, 455, 473, 477, 479, 505], 0.95, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([391, 393, 395, 397, 399, 401, 403, 405, 407, 409, 419, 421, 423, 425, 427, 463, 465, 467, 469, 471, 475, 481, 483, 485, 487, 489, 491, 493, 495, 497, 499, 501], 1.0, 'REPLACE')
bpy.data.objects['B'].modifiers.new(name='M', type='LAPLACIANSMOOTH')
bpy.data.objects['B'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['B'].modifiers['M'].lambda_factor = 2.0
bpy.data.objects['B'].modifiers['M'].iterations = 3
bpy.data.objects['B'].modifiers['M'].invert_vertex_group = True
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1)
bpy.context.object.name = 'C'
bpy.data.objects['C'].vertex_groups.new(name='G')
bpy.data.objects['C'].vertex_groups['G'].add([5, 10, 20, 25, 35, 40, 50, 55], 0.25, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([8, 13, 23, 28, 38, 43, 53, 58], 0.4, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([1, 11, 16, 26, 31, 41, 46, 56, 61], 0.5, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([4, 14, 19, 29, 34, 44, 49, 59], 0.6, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([2, 7, 17, 22, 32, 37, 47, 52], 0.75, 'REPLACE')
bpy.data.objects['C'].modifiers.new(name='M', type='LAPLACIANSMOOTH')
bpy.data.objects['C'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['C'].modifiers['M'].lambda_factor = 0.8
bpy.data.objects['C'].modifiers['M'].iterations = 3
bpy.data.objects['C'].modifiers['M'].use_volume_preserve = False
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'D'
bpy.data.objects['D'].vertex_groups.new(name='G')
bpy.data.objects['D'].vertex_groups['G'].add([0, 6, 35, 41], 0.15, 'REPLACE')
bpy.data.objects['D'].vertex_groups['G'].add([1, 5, 7, 13, 28, 34, 36, 40], 0.3, 'REPLACE')
bpy.data.objects['D'].vertex_groups['G'].add([2, 4, 37, 39], 0.35, 'REPLACE')
bpy.data.objects['D'].vertex_groups['G'].add([3, 14, 20, 21, 27, 38], 0.4, 'REPLACE')
bpy.data.objects['D'].vertex_groups['G'].add([8, 12, 29, 33], 0.45, 'REPLACE')
bpy.data.objects['D'].vertex_groups['G'].add([9, 11, 15, 19, 22, 26, 30, 32], 0.6, 'REPLACE')
bpy.data.objects['D'].vertex_groups['G'].add([10, 31], 0.65, 'REPLACE')
bpy.data.objects['D'].vertex_groups['G'].add([16, 18, 23, 25], 0.75, 'REPLACE')
bpy.data.objects['D'].vertex_groups['G'].add([17, 24], 0.9, 'REPLACE')
bpy.data.objects['D'].modifiers.new(name='M', type='LAPLACIANSMOOTH')
bpy.data.objects['D'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['D'].modifiers['M'].lambda_factor = 2.0
bpy.data.objects['D'].modifiers['M'].iterations = 3
bpy.data.objects['D'].modifiers['M'].lambda_border = 0.5
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1)
bpy.context.object.name = 'E'
bpy.data.objects['E'].vertex_groups.new(name='H')
bpy.data.objects['E'].vertex_groups['H'].add([7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31], 1.0, 'REPLACE')
bpy.data.objects['E'].modifiers.new(name='M', type='LAPLACIANSMOOTH')
bpy.data.objects['E'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['E'].modifiers['M'].lambda_factor = 2.0
bpy.data.objects['E'].modifiers['M'].iterations = 3
