# WP 4.7: export_apply=True with a Boolean in TRANSFER mode: the evaluated mesh's slots are
# the cube's material followed by the cutter's 'Blue', so the applied cube exports two
# primitives although its object has one slot; the cutter (no modifiers) exports its data.
# golden: startup=factory tier=T2 compare=glb:out/boolean.glb tol=1e-4
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=16, radius=0.6, depth=3, location=(0, 0, 0.3))
bpy.data.materials.new('Blue')
bpy.data.objects['Cylinder'].data.materials.append(bpy.data.materials['Blue'])
bpy.data.objects['Cube'].modifiers.new(name='Boolean', type='BOOLEAN')
bpy.data.objects['Cube'].modifiers['Boolean'].object = bpy.data.objects['Cylinder']
bpy.data.objects['Cube'].modifiers['Boolean'].solver = 'MANIFOLD'
bpy.data.objects['Cube'].modifiers['Boolean'].operation = 'DIFFERENCE'
bpy.data.objects['Cube'].modifiers['Boolean'].material_mode = 'TRANSFER'
bpy.ops.export_scene.gltf(filepath='out/boolean.glb', export_apply=True)
