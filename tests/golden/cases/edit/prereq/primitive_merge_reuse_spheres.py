# WP 2.2: an ico sphere (whose builder leaves no freed slot), then default cones (a zero
# radius kills the cap centre and collapses the ring: freed vertex, edge and face slots), then
# a plane, a cube, a grid and a circle in one Edit Mode session: each fills the freed slots
# first, in Blender's creation order (the cube and grid hand out UVs in face pool order).
# Spheres built while the edit mesh has freed slots are not covered: their builders
# (subdivide, extrude + remove_doubles) recreate elements in an order frap does not replay yet.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=0.5, location=(-3, 0, 0))
bpy.ops.mesh.primitive_cone_add(location=(3, 3, 0))
bpy.ops.mesh.primitive_cone_add(vertices=6, radius1=0, radius2=0.5, location=(3, 0, 0))
bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 3, 0))
bpy.ops.mesh.primitive_cube_add(size=0.5, location=(0, -3, 0))
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=2, size=1, location=(3, -3, 0))
bpy.ops.mesh.primitive_circle_add(vertices=5, fill_type='TRIFAN', location=(-3, -3, 0))
