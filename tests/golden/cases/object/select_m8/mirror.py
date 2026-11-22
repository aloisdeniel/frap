# WP 8.3.4: object.select_mirror: the objects whose names are the flipped names of the selected
# ones (BLI_string_flip_side_name: .L/.R, L_/R_, Left/Right, a trailing number stripped) are
# selected; without extend the originals are deselected (with both sides selected, the later
# one in base order keeps the selection); a hidden mirror object is still selected; no mirror
# keeps nothing selected; the active object never changes.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.empty_add(location=(1, 0, 0))
bpy.context.object.name = 'Hand.L'
bpy.ops.object.empty_add(location=(2, 0, 0))
bpy.context.object.name = 'Hand.R'
bpy.ops.object.empty_add(location=(3, 0, 0))
bpy.context.object.name = 'L_arm'
bpy.ops.object.empty_add(location=(4, 0, 0))
bpy.context.object.name = 'R_arm'
bpy.ops.object.empty_add(location=(5, 0, 0))
bpy.context.object.name = 'FootLeft.001'
bpy.ops.object.empty_add(location=(6, 0, 0))
bpy.context.object.name = 'FootRight'
bpy.ops.object.empty_add(location=(7, 0, 0))
bpy.context.object.name = 'eye.l'
bpy.ops.object.empty_add(location=(8, 0, 0))
bpy.context.object.name = 'eye.r'
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Hand.L'].select_set(True)
bpy.data.objects['L_arm'].select_set(True)
bpy.ops.object.select_mirror()
bpy.ops.transform.translate(value=(0, 0, 1))
bpy.ops.object.select_mirror(extend=True)
bpy.ops.transform.translate(value=(0, 0, 2))
bpy.ops.object.select_mirror()
bpy.ops.transform.translate(value=(0, 0, 4))
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['FootLeft.001'].select_set(True)
bpy.data.objects['eye.l'].hide_set(True)
bpy.data.objects['eye.r'].select_set(True)
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.select_mirror()
bpy.ops.transform.translate(value=(0, 0, 8))
bpy.data.objects['eye.l'].hide_set(False)
