# WP 3.9: a table modelled in Edit Mode and exported. The cube is flattened into a top, its
# bottom face is inset and extruded down into a pedestal, the top edges are bevelled; the
# pedestal faces get a second material (material_slot_assign on the selection). Round trip
# (§17.2).
# golden: startup=factory tier=T2 compare=glb:out/table.glb,glb:out/table_rt.glb glb_structure
import bpy
bpy.data.objects['Cube'].name = 'Table'
bpy.data.materials['Material'].name = 'Walnut'
bpy.data.materials['Walnut'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.35, 0.2, 0.1, 1.0)
bpy.data.materials.new('Iron')
bpy.data.materials['Iron'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Iron'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.2, 0.2, 0.22, 1.0)
bpy.ops.object.material_slot_add()
bpy.data.objects['Table'].material_slots[1].material = bpy.data.materials['Iron']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.resize(value=(1.5, 1.0, 0.1))
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_all(action='DESELECT')
#@frap    bpy.ops.frap.select_elements(type='FACE', indices=[4])
#@blender import bmesh; bm = bmesh.from_edit_mesh(bpy.context.object.data); bm.faces.ensure_lookup_table(); bm.faces[4].select_set(True); bmesh.update_edit_mesh(bpy.context.object.data)
bpy.ops.mesh.inset(thickness=0.9)
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={'value': (0, 0, -1.2)})
bpy.context.object.active_material_index = 1
bpy.ops.object.material_slot_assign()
bpy.ops.mesh.select_all(action='DESELECT')
#@frap    bpy.ops.frap.select_elements(type='FACE', indices=[5])
#@blender import bmesh; bm = bmesh.from_edit_mesh(bpy.context.object.data); bm.faces.ensure_lookup_table(); bm.faces[5].select_set(True); bmesh.update_edit_mesh(bpy.context.object.data)
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.bevel(offset=0.05, segments=2)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.export_scene.gltf(filepath='out/table.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/table.glb')
bpy.ops.export_scene.gltf(filepath='out/table_rt.glb')
