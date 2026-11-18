# WP 2.6: mesh.unsubdivide on meshes with triangles, boundaries and wire chains: a subdivided
# ico sphere, a subdivided plane (boundary fans), a circle without fill (wire chain), in edge
# select mode.
# golden: startup=factory compare=objects,mesh:Icosphere,mesh:Plane,mesh:Circle tier=T1
import bpy
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide()
bpy.ops.mesh.unsubdivide(iterations=1)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=3)
bpy.ops.mesh.unsubdivide()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_circle_add(vertices=16, location=(-3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.unsubdivide(iterations=2)
