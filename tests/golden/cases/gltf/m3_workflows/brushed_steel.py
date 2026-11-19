# WP 3.9: a brushed steel pan. KHR_materials_anisotropy from unlinked sockets (Anisotropic 0.7
# -> anisotropyStrength, Anisotropic Rotation 0.25 -> anisotropyRotation pi/2) on a fully
# metallic material; the handle has a rotation only (strength 0: no extension). Round trip
# (§17.2).
# golden: startup=factory tier=T2 compare=glb:out/pan.glb,glb:out/pan_rt.glb
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_cylinder_add(vertices=16, radius=1.0, depth=0.2)
bpy.data.objects['Cylinder'].name = 'Pan'
bpy.data.materials['Material'].name = 'Steel'
bpy.data.objects['Pan'].data.materials.append(bpy.data.materials['Steel'])
bpy.data.materials['Steel'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Steel'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.3
bpy.data.materials['Steel'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic'].default_value = 0.7
bpy.data.materials['Steel'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic Rotation'].default_value = 0.25
bpy.ops.mesh.primitive_cube_add(size=1, location=(1.6, 0, 0), scale=(1.2, 0.15, 0.1))
bpy.data.objects['Cube'].name = 'Handle'
bpy.data.materials.new('Grip')
bpy.data.objects['Handle'].data.materials.append(bpy.data.materials['Grip'])
bpy.data.materials['Grip'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic Rotation'].default_value = 0.1
bpy.ops.export_scene.gltf(filepath='out/pan.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/pan.glb')
bpy.ops.export_scene.gltf(filepath='out/pan_rt.glb')
