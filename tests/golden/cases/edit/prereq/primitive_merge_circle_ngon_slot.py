# M2 stabilization: the NGON circle builder (`bmo_create_circle_exec`) creates its centre vertex
# first and kills it when the fan is dissolved, leaving a freed vertex slot in the edit BMesh;
# the next primitive's first vertex takes it (the plane's first vertex lands at index 8, after
# the factory cube's vertices, not after the circle's). TRIFAN and NOTHING circles free nothing.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_circle_add(vertices=9, fill_type='NGON', location=(0, -3, 0))
bpy.ops.mesh.primitive_plane_add(size=0.5, location=(0, 0, 3))
bpy.ops.mesh.primitive_circle_add(vertices=5, fill_type='TRIFAN', location=(3, 0, 0))
bpy.ops.mesh.primitive_circle_add(vertices=6, fill_type='NGON', location=(-3, 0, 0))
bpy.ops.mesh.primitive_circle_add(vertices=4, fill_type='NOTHING', location=(0, 3, 0))
bpy.ops.mesh.primitive_cube_add(size=0.5, location=(3, 3, 0))
bpy.ops.object.mode_set(mode='OBJECT')
