# WP 2.10: mesh.wireframe with the defaults turns the cube into V40 E96 F48 (the original
# faces replaced); on a grid's middle faces with boundary insets, crease (creates
# crease_edge), relative offset and use_replace off; the wire faces are selected.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.wireframe()
bpy.data.meshes['Cube'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=3, size=2, location=(4, 0, 0), enter_editmode=True)
bpy.ops.frap.select_elements(type='FACE', indices=[1, 2, 4])
bpy.ops.mesh.wireframe(thickness=0.1, offset=0.3, use_crease=True, crease_weight=0.7, use_replace=False)
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.wireframe(thickness=0.05, use_relative_offset=True, use_even_offset=False, use_boundary=False)
bpy.data.meshes['Grid'].total_face_sel
