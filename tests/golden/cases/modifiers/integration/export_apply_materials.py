# WP 4.7: material indices made by modifiers with export_apply=True. Solidify's
# material_offset_rim moves the rim faces to slot 1 (the second material); on a one-slot mesh
# material_offset points past the last slot, which the add-on's get_material_from_idx maps to
# the last slot. A mesh whose only slot is empty falls back to the object's material slots
# (the OBJECT-linked 'Red'). Without export_apply the same objects export their data (one
# primitive each).
# golden: startup=factory tier=T2 compare=glb:out/applied.glb,glb:out/data.glb tol=1e-4 glb_structure
import bpy
bpy.data.materials.new(name='Red')
bpy.data.materials.new(name='Blue')
bpy.ops.mesh.primitive_plane_add(size=2, location=(3, 0, 0))
bpy.data.objects['Plane'].data.materials.append(bpy.data.materials['Red'])
bpy.data.objects['Plane'].data.materials.append(bpy.data.materials['Blue'])
bpy.data.objects['Plane'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Plane'].modifiers['Solidify'].material_offset_rim = 1
bpy.ops.mesh.primitive_plane_add(size=2, location=(-3, 0, 0))
bpy.data.objects['Plane.001'].data.materials.append(bpy.data.materials['Blue'])
bpy.data.objects['Plane.001'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Plane.001'].modifiers['Solidify'].material_offset = 1
bpy.ops.mesh.primitive_plane_add(size=2, location=(0, 3, 0))
bpy.data.objects['Plane.002'].data.materials.append(None)
bpy.data.objects['Plane.002'].material_slots[0].link = 'OBJECT'
bpy.data.objects['Plane.002'].material_slots[0].material = bpy.data.materials['Red']
bpy.data.objects['Plane.002'].modifiers.new(name='Triangulate', type='TRIANGULATE')
bpy.ops.export_scene.gltf(filepath='out/applied.glb', export_apply=True)
bpy.ops.export_scene.gltf(filepath='out/data.glb')
