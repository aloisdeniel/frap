# WP 2.13 workflow (table): a top and four legs added as primitives while in Edit Mode (each
# is merged into the edited mesh and becomes the only selection), then separated by loose
# parts into five objects.
# golden: startup=empty compare=objects,context,mesh:Table,mesh:Table.001,mesh:Table.004 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 0.75), scale=(1, 0.6, 0.04))
bpy.context.object.name = 'Table'
bpy.context.object.data.name = 'Table'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_cube_add(size=0.1, location=(0.9, 0.5, 0.36), scale=(1, 1, 7.2))
bpy.data.meshes['Table'].total_vert_sel
bpy.ops.mesh.primitive_cube_add(size=0.1, location=(-0.9, 0.5, 0.36), scale=(1, 1, 7.2))
bpy.ops.mesh.primitive_cube_add(size=0.1, location=(-0.9, -0.5, 0.36), scale=(1, 1, 7.2))
bpy.ops.mesh.primitive_cube_add(size=0.1, location=(0.9, -0.5, 0.36), scale=(1, 1, 7.2))
bpy.data.meshes['Table'].total_vert_sel
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.separate(type='LOOSE')
bpy.ops.object.mode_set(mode='OBJECT')
