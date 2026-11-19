# WP 3.9: a house with one mesh and several materials. The walls cube is assigned per face in
# Edit Mode (object.material_slot_add, material_slot_assign on selected faces): roof faces,
# door face, and an empty slot (a primitive without material); one primitive per used slot in
# slot order. export_materials='NONE' writes one primitive for the whole mesh. Round trip
# (§17.2) of both exports.
# golden: startup=factory tier=T2 compare=glb:out/house.glb,glb:out/house_nomat.glb,glb:out/house_rt.glb,glb:out/house_nomat_rt.glb
import bpy
bpy.data.objects['Cube'].name = 'House'
bpy.data.materials['Material'].name = 'Plaster'
bpy.data.materials.new('Tiles')
bpy.data.materials['Tiles'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.6, 0.15, 0.1, 1.0)
bpy.data.materials.new('Door')
bpy.data.materials['Door'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.3, 0.2, 0.1, 1.0)
bpy.ops.object.material_slot_add()
bpy.data.objects['House'].material_slots[1].material = bpy.data.materials['Tiles']
bpy.ops.object.material_slot_add()
bpy.data.objects['House'].material_slots[2].material = bpy.data.materials['Door']
bpy.ops.object.material_slot_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_all(action='DESELECT')
#@frap    bpy.ops.frap.select_elements(type='FACE', indices=[5])
#@blender import bmesh; bm = bmesh.from_edit_mesh(bpy.context.object.data); bm.faces.ensure_lookup_table(); bm.faces[5].select_set(True); bmesh.update_edit_mesh(bpy.context.object.data)
bpy.context.object.active_material_index = 1
bpy.ops.object.material_slot_assign()
bpy.ops.mesh.select_all(action='DESELECT')
#@frap    bpy.ops.frap.select_elements(type='FACE', indices=[1])
#@blender import bmesh; bm = bmesh.from_edit_mesh(bpy.context.object.data); bm.faces.ensure_lookup_table(); bm.faces[1].select_set(True); bmesh.update_edit_mesh(bpy.context.object.data)
bpy.context.object.active_material_index = 2
bpy.ops.object.material_slot_assign()
bpy.ops.mesh.select_all(action='DESELECT')
#@frap    bpy.ops.frap.select_elements(type='FACE', indices=[4])
#@blender import bmesh; bm = bmesh.from_edit_mesh(bpy.context.object.data); bm.faces.ensure_lookup_table(); bm.faces[4].select_set(True); bmesh.update_edit_mesh(bpy.context.object.data)
bpy.context.object.active_material_index = 3
bpy.ops.object.material_slot_assign()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.export_scene.gltf(filepath='out/house.glb')
bpy.ops.export_scene.gltf(filepath='out/house_nomat.glb', export_materials='NONE')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/house.glb')
bpy.ops.export_scene.gltf(filepath='out/house_rt.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/house_nomat.glb')
bpy.ops.export_scene.gltf(filepath='out/house_nomat_rt.glb')
