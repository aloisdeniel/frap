# WP 8.3.4: object.select_random: the visible, selectable objects shuffled by seed (BLI_rng),
# the first int(count * ratio) selected or deselected, per seed and ratio; hidden and
# unselectable objects are not candidates. Each step's selection is recorded by a translation of
# the selected objects (no parents).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.empty_add(location=(1, 0, 0))
bpy.ops.object.empty_add(location=(2, 0, 0))
bpy.ops.object.empty_add(location=(3, 0, 0))
bpy.ops.object.empty_add(location=(4, 0, 0))
bpy.ops.object.empty_add(location=(5, 0, 0))
bpy.ops.object.empty_add(location=(6, 0, 0))
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.select_random()
bpy.ops.transform.translate(value=(0, 0, 1))
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.select_random(seed=1)
bpy.ops.transform.translate(value=(0, 0, 2))
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.select_random(seed=2, ratio=0.3)
bpy.ops.transform.translate(value=(0, 0, 4))
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.select_random(seed=7, ratio=0.6, action='DESELECT')
bpy.ops.transform.translate(value=(0, 0, 8))
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Empty.002'].hide_set(True)
bpy.data.objects['Empty.003'].hide_select = True
bpy.ops.object.select_random(seed=255, ratio=0.75)
bpy.ops.transform.translate(value=(0, 0, 16))
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.select_random(seed=12345, ratio=1.0)
bpy.ops.transform.translate(value=(0, 0, 32))
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.select_random(seed=3, ratio=0.0)
bpy.ops.object.select_random(seed=4, ratio=0.9)
