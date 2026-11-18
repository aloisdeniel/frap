# WP 2.2: mesh.primitive_*_add in Edit Mode merges the new geometry into the edit mesh, at the
# placement relative to the edit object, and selects only the new elements (the selection
# history is cleared); no object is added. The cylinder comes last: its builder frees pool
# slots, which Blender would reuse for the next primitive (see the edit_merge.rs notes).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[2])
bpy.ops.mesh.primitive_cube_add(size=0.5, location=(3, 0, 0))
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 3, 0))
bpy.ops.mesh.primitive_circle_add(vertices=6, radius=0.5, location=(0, 0, 3))
bpy.ops.mesh.primitive_circle_add(vertices=5, fill_type='TRIFAN', location=(0, -3, 0))
bpy.ops.mesh.primitive_grid_add(x_subdivisions=2, y_subdivisions=3, size=1, location=(-3, 0, 0))
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.primitive_monkey_add(size=0.5, location=(3, -3, 0))
bpy.ops.mesh.primitive_cylinder_add(vertices=8, radius=0.5, depth=1, location=(3, 3, 0))
bpy.context.mode
