# WP 3.8: uv.unwrap only touches the selected faces (a cylinder's side cut by one seam, its
# caps unselected keep their UVs), then packs them with fill_holes off; a plane in Object
# Mode is left alone, multi-object Edit Mode unwraps both selected meshes.
# golden: startup=empty compare=objects,context,mesh:Cylinder,mesh:Plane tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(0, 0, 0))
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.data.objects['Cylinder'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(object='Cylinder', type='EDGE', indices=[0])
bpy.ops.mesh.mark_seam()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(object='Cylinder', type='FACE', indices=[0, 1, 2, 3, 4, 5, 6, 7])
bpy.ops.frap.select_elements(object='Plane', type='FACE', indices=[0], action='ADD')
bpy.ops.uv.unwrap(method='CONFORMAL', fill_holes=False)
bpy.ops.object.mode_set(mode='OBJECT')
