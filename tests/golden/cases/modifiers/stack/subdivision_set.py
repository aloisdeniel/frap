# WP 4.1 (T0): object.subdivision_set adds "Subdivision" (before the pinned Smooth by Angle) or
# sets/offsets the first SUBSURF's levels (clamped at 0), warns for objects without modifiers,
# and refuses an empty selection, relative=True with level 0, and ensure_modifier=False
# without a SUBSURF, where only objects in Object Mode count (Edit Mode: CANCELLED, while
# ensure_modifier=True still sets the levels). Levels are encoded in a probe's location.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects.new('Probe', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Probe'])
bpy.ops.object.subdivision_set(level=2, ensure_modifier=False)
bpy.ops.object.shade_auto_smooth()
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.subdivision_set(level=2)
bpy.data.objects['Probe'].location.x = bpy.data.objects['Cube'].modifiers['Subdivision'].levels
bpy.ops.object.subdivision_set(level=-1, relative=True)
bpy.data.objects['Probe'].location.y = bpy.data.objects['Cube'].modifiers['Subdivision'].levels
bpy.ops.object.subdivision_set(level=-5, relative=True)
bpy.data.objects['Probe'].location.z = bpy.data.objects['Cube'].modifiers['Subdivision'].levels
bpy.ops.object.subdivision_set(level=0, relative=True)
bpy.ops.object.subdivision_set(level=3, ensure_modifier=False)
bpy.data.objects['Probe'].scale.x = bpy.data.objects['Cube'].modifiers['Subdivision'].levels
bpy.ops.object.subdivision_set(level=-4)
bpy.data.objects['Probe'].scale.y = bpy.data.objects['Cube'].modifiers['Subdivision'].levels
bpy.ops.object.subdivision_set(level=20)
bpy.data.objects['Probe'].scale.z = bpy.data.objects['Cube'].modifiers['Subdivision'].levels
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.subdivision_set(level=2)
bpy.ops.mesh.primitive_cylinder_add(location=(3, 0, 0))
bpy.ops.object.subdivision_set(level=1, relative=True)
bpy.data.objects['Probe'].rotation_euler.x = bpy.data.objects['Cylinder'].modifiers['Subdivision'].levels
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.subdivision_set(level=3, ensure_modifier=False)
bpy.data.objects['Probe'].rotation_euler.y = bpy.data.objects['Cylinder'].modifiers['Subdivision'].levels
bpy.ops.object.subdivision_set(level=2)
bpy.data.objects['Probe'].rotation_euler.z = bpy.data.objects['Cylinder'].modifiers['Subdivision'].levels
bpy.ops.object.mode_set(mode='OBJECT')
