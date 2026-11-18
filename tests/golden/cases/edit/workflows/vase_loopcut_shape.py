# WP 2.13 workflow (vase): an open cylinder (top cap deleted) with one loop cut slid towards
# the top with mesh.loopcut_slide, a second plain loop cut below it, each new loop scaled to
# shape the profile, then given thickness with mesh.solidify.
# golden: startup=empty compare=objects,context,mesh:Cylinder tier=T1 order=canonical tol=1e-4
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=16, radius=0.3, depth=1, location=(0, 0, 0.5))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_by_normal(direction=(0, 0, 1))
bpy.ops.mesh.delete(type='FACE')
bpy.ops.mesh.loopcut_slide(MESH_OT_loopcut={"number_cuts": 1, "edge_index": 2, "object_index": 0}, TRANSFORM_OT_edge_slide={"value": 0.5})
bpy.data.meshes['Cylinder'].total_edge_sel
bpy.ops.transform.resize(value=(0.6, 0.6, 1))
bpy.ops.mesh.loopcut(number_cuts=1, edge_index=2, object_index=0)
bpy.ops.transform.resize(value=(1.4, 1.4, 1))
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.solidify(thickness=0.02)
