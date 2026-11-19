# WP 3.8: uv.pack_islands over two objects in Edit Mode (a smart-projected UV sphere and a
# cube, packed together, margin FRACTION): T3 invariants (UVs in the unit square, islands
# disjoint).
# golden: startup=empty compare=objects,context,mesh:Sphere,mesh:Cube tier=T3 check=uv01,uv_no_overlap
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6)
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0), scale=(1, 0.5, 2))
bpy.data.objects['Sphere'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.smart_project()
bpy.ops.uv.pack_islands(margin_method='FRACTION', margin=0.02)
bpy.ops.object.mode_set(mode='OBJECT')
