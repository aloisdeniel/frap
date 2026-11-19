# WP 4.7: exporting from Edit Mode with export_apply=True leaves Edit Mode first (the
# add-on's `export.save`), so the evaluated mesh is built from the edited geometry; the
# Simple Deform and Displace stack on the cube and a Cast on the sphere are applied in order.
# golden: startup=factory tier=T2 compare=glb:out/edited.glb,objects,context tol=1e-4
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=10, ring_count=6, location=(3, 0, 0))
bpy.data.objects['Sphere'].modifiers.new(name='Cast', type='CAST')
bpy.data.objects['Sphere'].modifiers['Cast'].factor = 0.8
bpy.data.objects['Cube'].modifiers.new(name='Twist', type='SIMPLE_DEFORM')
bpy.data.objects['Cube'].modifiers['Twist'].angle = 0.5
bpy.data.objects['Cube'].modifiers.new(name='Displace', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['Displace'].strength = 0.25
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.resize(value=(1, 1, 2))
bpy.ops.export_scene.gltf(filepath='out/edited.glb', export_apply=True)
