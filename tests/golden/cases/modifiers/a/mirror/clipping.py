# WP 4.2: MIRROR `use_clip` constrains Edit Mode tools, never the evaluation: the half cube
# (its -X face on the plane) is extruded from the plane face and smoothed; the vertices on the
# mirror plane stay on it, and the evaluated mirror welds them with their images.
# golden: startup=factory compare=mesh:Cube,evaluated:Cube tier=T2
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.translate(value=(1, 0, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].modifiers.new(name='Mirror', type='MIRROR')
bpy.data.objects['Cube'].modifiers['Mirror'].use_clip = True
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.mesh.extrude_region()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.vertices_smooth(factor=0.5)
bpy.ops.object.mode_set(mode='OBJECT')
