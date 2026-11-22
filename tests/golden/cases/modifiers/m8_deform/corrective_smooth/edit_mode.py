# WP 8.3.12: CORRECTIVE_SMOOTH (`rest_source='ORCO'`, shown in Edit Mode) after a DISPLACE, in
# Edit Mode after a subdivision: the rest shape is the edit mesh's coordinates
# (`BKE_editmesh_vert_coords_alloc_orco`), not the mesh datablock's (whose vertex count no
# longer matches).
# golden: startup=empty compare=evaluated:A tier=T2
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.context.object.name = 'A'
bpy.data.objects['A'].modifiers.new(name='Push', type='DISPLACE')
bpy.data.objects['A'].modifiers['Push'].strength = 0.3
bpy.data.objects['A'].modifiers['Push'].show_in_editmode = True
bpy.data.objects['A'].modifiers.new(name='M', type='CORRECTIVE_SMOOTH')
bpy.data.objects['A'].modifiers['M'].show_in_editmode = True
bpy.data.objects['A'].modifiers['M'].iterations = 4
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=1)
