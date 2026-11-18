# WP 2.4: frap.select_by_normal normalises the direction in double precision: a component of
# 1e-30 (whose square underflows single precision) or 1e20 (whose square overflows it) still
# gives a unit direction, and 1e40 is clamped to FLT_MAX by the property, as RNA does.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_by_normal(direction=(0, 0, 1e-30))
bpy.ops.frap.select_by_normal(direction=(1e20, 0, 0), extend=True)
bpy.ops.frap.select_by_normal(direction=(0, -1e40, 0), extend=True)
