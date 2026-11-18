# WP 2.2: a UV sphere merged into an edit mesh, ending in Edit Mode with only the sphere
# selected. Compared canonically: Blender's UV-sphere edge and face order changes between runs
# (deviation D-19), so the selection is matched through the canonical element mapping.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 order=canonical
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=5, radius=0.5, location=(2, 0, 0))
bpy.data.meshes['Cube'].total_vert_sel
