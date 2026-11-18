# WP 2.12: an object scaled to zero along one axis: the pseudo-inverse of its matrix lets the
# selection move along the other axes, and the pivot uses the fallback inverse.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].scale = (1, 0, 2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1])
bpy.ops.transform.translate(value=(0.5, 0.5, 0.5))
bpy.ops.transform.rotate(value=0.3, orient_axis='X')
bpy.ops.transform.resize(value=(2, 2, 2))
