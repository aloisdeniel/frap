# M2 stabilization round 3 (D-77 family): mesh.subdivide(smoothness=1) on a spun profile.
# Positions agree with Blender within 1e-6, but the corner normals of the small non-planar
# faces differ by up to ~5e-5 (1-ulp position differences from the rotation and smooth
# interpolation float chains, amplified on small faces), above the default 1e-5. This pins
# the tolerance at 1e-4; T2 because spin's side-face order is not reproducible.
# golden: startup=factory compare=objects,context,mesh:Circle tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_circle_add(vertices=8, radius=0.5, location=(1.5, 0, 0), rotation=(1.5707963, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.spin(steps=12, angle=3.1415926, center=(0, 0, 0), axis=(0, 0, 1))
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=2, smoothness=1)
bpy.ops.object.mode_set(mode='OBJECT')
