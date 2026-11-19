# WP 3.8: uv.sphere_project on the default cube (factory viewport, D-10), on a UV sphere
# aligned to the object with fan poles (T2: a UV sphere's edit-mode element order is canonical
# only), and on a cylinder viewed from the poles with a seam kept and clipped to bounds.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Sphere,mesh:Cylinder tier=T2
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.sphere_project()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, location=(0, 4, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.sphere_project(direction='ALIGN_TO_OBJECT', pole='FAN')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cylinder_add(vertices=10, location=(4, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[2])
bpy.ops.mesh.mark_seam()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.sphere_project(direction='VIEW_ON_POLES', seam=True, clip_to_bounds=True)
bpy.ops.object.mode_set(mode='OBJECT')
