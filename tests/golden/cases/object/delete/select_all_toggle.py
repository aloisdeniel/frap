# WP 1.19 (T0), object.select_all, kept with the delete cases (the select golden area belongs to
# WP 1.20): TOGGLE deselects everything when any visible object is selected (the factory Cube),
# then selects every visible object; the active object stays Cube.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.select_all()
bpy.ops.object.select_all(action='TOGGLE')
