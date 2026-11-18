# WP 2.12: transform.transform(mode=…): a set value is an array, so it constrains to every axis
# (ROTATION then scales by cos(angle); PUSHPULL with a value reads an uninitialized axis in
# Blender, so it is not compared); TRANSLATION/ROTATION/RESIZE also run in Object Mode; the Edit
# Mode modes TOSPHERE, SHRINKFATTEN, CREASE, VERTEX_CREASE, BWEIGHT, MIRROR.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.ops.transform.transform(mode='TRANSLATION', value=(1, 0, 0.5, 0))
bpy.ops.transform.transform(mode='RESIZE', value=(2, 1, 1, 0))
bpy.ops.transform.transform(mode='ROTATION', value=(0.5, 0, 0, 0))
bpy.ops.transform.transform(mode='ROTATION', value=(0.5, 0, 0, 0), orient_axis='X')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2, 3])
bpy.ops.transform.transform(mode='TRANSLATION', value=(0, 0, 0.5, 0))
bpy.ops.transform.transform(mode='ROTATION', value=(0.3, 0, 0, 0))
bpy.ops.transform.transform(mode='RESIZE', value=(1.2, 0.8, 1, 0), use_proportional_edit=True, proportional_size=3)
bpy.ops.transform.transform(mode='TOSPHERE', value=(0.5, 0, 0, 0))
bpy.ops.transform.transform(mode='SHRINKFATTEN', value=(0.2, 0, 0, 0))
bpy.ops.transform.transform(mode='MIRROR', value=(1, 1, 1, 0))
bpy.ops.transform.transform(mode='MIRROR', constraint_axis=(False, False, True))
bpy.ops.transform.transform(mode='CREASE', value=(0.5, 0, 0, 0))
bpy.ops.transform.transform(mode='BWEIGHT', value=(0.25, 0, 0, 0))
bpy.ops.transform.transform(mode='VERTEX_CREASE', value=(0.75, 0, 0, 0))
bpy.ops.transform.transform(mode='TRANSLATION')
