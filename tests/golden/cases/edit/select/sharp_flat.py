# WP 2.4: mesh.edges_select_sharp on a cylinder (cap and side edges are sharp, side seams are
# not at 60 degrees) in vertex mode and in face mode (EDBM_selectmode_convert to faces), and
# mesh.faces_select_linked_flat growing over the flat cap and over the sides at a wider angle.
# golden: startup=factory compare=objects,context,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.edges_select_sharp()
bpy.data.meshes['Cylinder'].total_edge_sel
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.edges_select_sharp(sharpness=1.0)
bpy.data.meshes['Cylinder'].total_edge_sel
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.edges_select_sharp(sharpness=1.0)
bpy.data.meshes['Cylinder'].total_face_sel
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.faces_select_linked_flat()
bpy.data.meshes['Cylinder'].total_face_sel
bpy.ops.mesh.faces_select_linked_flat(sharpness=0.9)
bpy.data.meshes['Cylinder'].total_face_sel
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.faces_select_linked_flat()
