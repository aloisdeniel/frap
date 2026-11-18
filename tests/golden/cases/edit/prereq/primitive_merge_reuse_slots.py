# WP 2.2: the cylinder and cone builder kills elements (cap centres, collapsed rings, dissolved
# fans), leaving freed pool slots in the edit BMesh; the next primitives of the same Edit Mode
# session fill them first (the cube's first vertices take the cylinder's freed slots), and a
# second cylinder's caps are dissolved in the slot order its faces got (four table legs).
# frap runs the builder in the edit BMesh, as Blender does.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_cylinder_add(vertices=8, radius=0.5, depth=1, location=(3, 3, 0))
bpy.ops.mesh.primitive_cube_add(size=0.5, location=(3, 0, 0))
bpy.ops.mesh.primitive_cylinder_add(vertices=8, radius=0.1, depth=1, location=(1, 1, -1))
bpy.ops.mesh.primitive_cylinder_add(vertices=8, radius=0.1, depth=1, location=(-1, 1, -1))
bpy.ops.mesh.primitive_cylinder_add(vertices=12, radius=0.1, depth=1, location=(1, -1, -1))
bpy.ops.mesh.primitive_cone_add(vertices=5, radius1=0.5, radius2=0, depth=1, location=(-3, 0, 0))
bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 3, 0))
bpy.ops.mesh.primitive_cylinder_add(vertices=6, end_fill_type='TRIFAN', location=(0, -3, 0))
bpy.ops.mesh.primitive_cylinder_add(vertices=5, end_fill_type='NOTHING', location=(-3, -3, 0))
bpy.ops.mesh.primitive_circle_add(vertices=7, fill_type='NGON', location=(0, 0, 3))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.mode_set(mode='EDIT')
