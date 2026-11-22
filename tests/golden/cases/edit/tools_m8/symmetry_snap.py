# WP 8.3.3: mesh.symmetry_snap: the monkey jittered, then snapped back with the defaults
# (NEGATIVE_X, factor 0.5, centre vertices on the axis), with another axis and direction, a
# small threshold (unpaired vertices: WARNING "… failed"), use_center off, and use_topology
# (topological pairs); a cube moved off-centre along Y; nothing selected: CANCELLED.
# golden: startup=factory compare=objects,context,mesh:Suzanne,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2, 3])
bpy.ops.transform.translate(value=(0.0, 0.3, 0.0))
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.symmetry_snap(direction='POSITIVE_Y', threshold=0.5, factor=0.25)
bpy.ops.mesh.symmetry_snap(direction='NEGATIVE_Z', threshold=1.0, factor=1.0, use_center=False)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.symmetry_snap()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_monkey_add(location=(4, 0, 0), enter_editmode=True)
bpy.ops.transform.vertex_random(offset=0.01, seed=1)
bpy.ops.mesh.symmetry_snap()
bpy.ops.transform.vertex_random(offset=0.02, seed=2)
bpy.ops.mesh.symmetry_snap(direction='POSITIVE_X', threshold=0.015, factor=0.8)
bpy.ops.mesh.symmetry_snap(direction='POSITIVE_X', threshold=0.1, use_center=False)
bpy.ops.transform.vertex_random(offset=0.05, seed=4)
bpy.ops.mesh.symmetry_snap(use_topology=True, factor=0.3)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[5, 7, 100, 101, 102, 250, 300, 400])
bpy.ops.mesh.symmetry_snap(direction='POSITIVE_X', threshold=0.2, factor=0.0)
