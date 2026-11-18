# WP 4.3: BUILD at frame 1: half of Suzanne's faces (start -49), a reversed quarter of a grid,
# a random half of an ico sphere (seed 7), and the edges branch on an edge-only circle.
# golden: startup=empty compare=evaluated:Suzanne,evaluated:Grid,evaluated:Icosphere,evaluated:Circle tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.data.objects['Suzanne'].modifiers.new(name='M', type='BUILD')
bpy.data.objects['Suzanne'].modifiers['M'].frame_start = -49
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=4, location=(3, 0, 0))
bpy.data.objects['Grid'].modifiers.new(name='M', type='BUILD')
bpy.data.objects['Grid'].modifiers['M'].frame_start = -24
bpy.data.objects['Grid'].modifiers['M'].use_reverse = True
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, location=(6, 0, 0))
bpy.data.objects['Icosphere'].modifiers.new(name='M', type='BUILD')
bpy.data.objects['Icosphere'].modifiers['M'].frame_start = -29
bpy.data.objects['Icosphere'].modifiers['M'].frame_duration = 60
bpy.data.objects['Icosphere'].modifiers['M'].use_random_order = True
bpy.data.objects['Icosphere'].modifiers['M'].seed = 7
bpy.ops.mesh.primitive_circle_add(vertices=10, location=(9, 0, 0))
bpy.data.objects['Circle'].modifiers.new(name='M', type='BUILD')
bpy.data.objects['Circle'].modifiers['M'].frame_start = -59
bpy.data.objects['Circle'].modifiers['M'].use_random_order = True
bpy.data.objects['Circle'].modifiers['M'].seed = 3
