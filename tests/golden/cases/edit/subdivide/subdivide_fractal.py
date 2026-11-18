# WP 2.6: mesh.subdivide with fractal displacement (BLI_rng offsets from the seed, new-Perlin
# turbulence), along the normal and free, two seeds.
# golden: startup=factory compare=objects,mesh:Cube,mesh:Grid tier=T1 tol=1e-4
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=2, fractal=1.5, seed=3)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(fractal=4.0, fractal_along_normal=1.0, seed=17)
