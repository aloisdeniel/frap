# WP 8.3.7: `object.update_shapes` (`ED_mesh_shapes_join_objects_exec` without adding keys): the
# blocks named after the other selected meshes take their evaluated positions (a modifier
# included) in their local space; `use_mirror` updates the block of the other side's name
# (`Arm.R` → `Arm.L`) and mirrors it along X; the refusals (no other mesh, another vertex count:
# CANCELLED) and errors (a name without `.L`/`.R` with `use_mirror`, no name matching a block);
# the poll (a key needed, Object Mode). Single blocks are compared through linked duplicates
# with `show_only_shape_key`.
# golden: startup=factory tier=T1 compare=objects,evaluated:Cube.001,evaluated:Cube.002,evaluated:Cube.003
import bpy
bpy.ops.object.update_shapes()
bpy.ops.mesh.primitive_cube_add(size=1, location=(3, 0, 0))
bpy.context.object.name = 'Arm.L'
bpy.ops.mesh.primitive_cube_add(size=1, location=(-3, 0, 0))
bpy.context.object.name = 'Body'
bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 3, 0), rotation=(0.3, 0, 0))
bpy.context.object.name = 'Arm.R'
bpy.ops.mesh.primitive_cube_add(size=1.5, location=(0, -3, 0))
bpy.context.object.name = 'Nope'
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(0, 0, 3))
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Arm.L'].select_set(True)
bpy.data.objects['Body'].select_set(True)
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.join_shapes()
bpy.data.meshes['Cube.001'].vertices[0].co = (1.0, 0.75, 0.5)
bpy.data.meshes['Cube.001'].vertices[4].co = (-0.25, 0.5, 0.5)
bpy.data.objects['Body'].modifiers.new(name='Push', type='DISPLACE')
bpy.data.objects['Body'].modifiers['Push'].strength = 0.4
bpy.ops.object.update_shapes()
bpy.data.objects['Camera'].location = (bpy.data.objects['Cube'].active_shape_key_index, bpy.data.shape_keys['Key'].key_blocks['Arm.L'].value, 0)
bpy.ops.object.update_shapes(use_mirror=True)
bpy.data.objects['Body'].select_set(False)
bpy.data.objects['Arm.L'].select_set(False)
bpy.data.objects['Arm.R'].select_set(True)
bpy.data.meshes['Cube.003'].vertices[1].co = (0.5, 0.25, -0.5)
bpy.ops.object.update_shapes(use_mirror=True)
bpy.ops.object.update_shapes()
bpy.data.objects['Arm.R'].select_set(False)
bpy.ops.object.update_shapes()
bpy.data.objects['Nope'].select_set(True)
bpy.ops.object.update_shapes()
bpy.data.objects['Sphere'].select_set(True)
bpy.ops.object.update_shapes()
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.update_shapes()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Body'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Body']
bpy.ops.object.update_shapes()
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.duplicate(linked=True)
bpy.data.objects['Cube.001'].show_only_shape_key = True
bpy.data.objects['Cube.001'].active_shape_key_index = 1
bpy.ops.object.duplicate(linked=True)
bpy.data.objects['Cube.002'].show_only_shape_key = True
bpy.data.objects['Cube.002'].active_shape_key_index = 2
bpy.ops.object.duplicate(linked=True)
bpy.data.objects['Cube.003'].show_only_shape_key = True
bpy.data.objects['Cube.003'].active_shape_key_index = 0
