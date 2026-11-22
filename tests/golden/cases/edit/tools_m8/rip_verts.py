# WP 8.3.3: mesh.rip / mesh.rip_move of one selected vertex, headless-extended (D-153): the
# mouse is the 3D cursor seen in the factory Layout 3D View. A cube corner ripped towards
# two sides (on the cube and a duplicate), a grid's inner vertex (the two faces at the edge
# nearest to the cursor get the new vertex), a boundary vertex, a bowtie vertex (the fan
# nearest to the cursor is split off), the middle of a wire chain, and rip_move with a
# translation. `blender -b` cannot run rip, so the Blender side splits the vertex the same
# way (`bmesh.utils.vert_separate`) and selects the copy an interactive Blender 5.2.4
# selects with the mouse over the cursor (recorded with `--enable-event-simulate`,
# tests/golden/cases/edit/tools_m8/README.md); the elements are created in another order,
# so the meshes compare canonically.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001,mesh:Grid,mesh:Grid.001,mesh:Grid.002,mesh:Grid.003,mesh:Grid.004 tier=T1 order=canonical
import bpy
bpy.ops.object.duplicate()
bpy.ops.object.select_all(action='DESELECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.context.scene.cursor.location = (1.5, 0.5, 1.2)
#@frap bpy.ops.mesh.rip()
#@blender import bmesh; from mathutils import Vector; me = bpy.context.object.data; bm = bmesh.from_edit_mesh(me); bm.verts.ensure_lookup_table(); E = lambda a, b: next(e for e in bm.edges if {v.index for v in e.verts} == {a, b}); bmesh.utils.vert_separate(bm.verts[0], [E(0, 1), E(0, 4)]); bm.verts.index_update(); S = [v for v in bm.verts if (v.co - Vector((1, 1, 1))).length < 1e-6 and not any(e.other_vert(v).index == 2 for e in v.link_edges)]; [x.select_set(False) for x in list(bm.verts) + list(bm.edges) + list(bm.faces)]; bm.select_history.clear(); S[0].select_set(True); bm.select_history.add(S[0]); bm.select_flush_mode(); bmesh.update_edit_mesh(me)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.001']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.context.scene.cursor.location = (0.2, 1.4, 0.9)
#@frap bpy.ops.mesh.rip()
#@blender import bmesh; from mathutils import Vector; me = bpy.context.object.data; bm = bmesh.from_edit_mesh(me); bm.verts.ensure_lookup_table(); E = lambda a, b: next(e for e in bm.edges if {v.index for v in e.verts} == {a, b}); bmesh.utils.vert_separate(bm.verts[0], [E(0, 1), E(0, 2)]); bm.verts.index_update(); S = [v for v in bm.verts if (v.co - Vector((1, 1, 1))).length < 1e-6 and any(e.other_vert(v).index == 4 for e in v.link_edges)]; [x.select_set(False) for x in list(bm.verts) + list(bm.edges) + list(bm.faces)]; bm.select_history.clear(); S[0].select_set(True); bm.select_history.add(S[0]); bm.select_flush_mode(); bmesh.update_edit_mesh(me)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=2, location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[12])
bpy.context.scene.cursor.location = (0.1, 0.4, 0.0)
#@frap bpy.ops.mesh.rip()
#@blender import bmesh; from mathutils import Vector; me = bpy.context.object.data; bm = bmesh.from_edit_mesh(me); bm.verts.ensure_lookup_table(); E = lambda a, b: next(e for e in bm.edges if {v.index for v in e.verts} == {a, b}); bmesh.utils.vert_separate(bm.verts[12], [E(12, 11), E(12, 13)]); bm.verts.index_update(); S = [v for v in bm.verts if v.co.length < 1e-6 and any(e.other_vert(v).index == 17 for e in v.link_edges)]; [x.select_set(False) for x in list(bm.verts) + list(bm.edges) + list(bm.faces)]; bm.select_history.clear(); S[0].select_set(True); bm.select_history.add(S[0]); bm.select_flush_mode(); bmesh.update_edit_mesh(me)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=2, location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[2])
bpy.context.scene.cursor.location = (0.3, -0.9, 0.0)
#@frap bpy.ops.mesh.rip()
#@blender import bmesh; from mathutils import Vector; me = bpy.context.object.data; bm = bmesh.from_edit_mesh(me); bm.verts.ensure_lookup_table(); E = lambda a, b: next(e for e in bm.edges if {v.index for v in e.verts} == {a, b}); bmesh.utils.vert_separate(bm.verts[2], [E(2, 7)]); bm.verts.index_update(); S = [v for v in bm.verts if (v.co - Vector((0, -1, 0))).length < 1e-6 and any(e.other_vert(v).index == 3 for e in v.link_edges)]; [x.select_set(False) for x in list(bm.verts) + list(bm.edges) + list(bm.faces)]; bm.select_history.clear(); S[0].select_set(True); bm.select_history.add(S[0]); bm.select_flush_mode(); bmesh.update_edit_mesh(me)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=2, y_subdivisions=2, size=2, location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[1, 2])
bpy.ops.mesh.delete(type='FACE')
bpy.ops.frap.select_elements(type='VERT', indices=[3])
bpy.context.scene.cursor.location = (0.5, 0.5, 0.0)
#@frap bpy.ops.mesh.rip()
#@blender import bmesh; from mathutils import Vector; me = bpy.context.object.data; bm = bmesh.from_edit_mesh(me); bm.verts.ensure_lookup_table(); bmesh.utils.vert_separate(bm.verts[3], []); S = [v for v in bm.verts if v.co.length < 1e-6 and any((e.other_vert(v).co - Vector((1, 0, 0))).length < 1e-6 for e in v.link_edges)]; [x.select_set(False) for x in list(bm.verts) + list(bm.edges) + list(bm.faces)]; bm.select_history.clear(); S[0].select_set(True); bm.select_history.add(S[0]); bm.select_flush_mode(); bmesh.update_edit_mesh(me)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=2, location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[24])
bpy.ops.mesh.extrude_vertices_move(TRANSFORM_OT_translate={'value': (0.0, 0.0, 1.0)})
bpy.ops.mesh.extrude_vertices_move(TRANSFORM_OT_translate={'value': (0.5, 0.0, 0.0)})
bpy.ops.frap.select_elements(type='VERT', indices=[25])
bpy.context.scene.cursor.location = (1.5, 1.0, 1.0)
#@frap bpy.ops.mesh.rip()
#@blender import bmesh; from mathutils import Vector; me = bpy.context.object.data; bm = bmesh.from_edit_mesh(me); bm.verts.ensure_lookup_table(); bmesh.utils.vert_separate(bm.verts[25], []); S = [v for v in bm.verts if (v.co - Vector((1, 1, 1))).length < 1e-6 and any((e.other_vert(v).co - Vector((1.5, 1, 1))).length < 1e-6 for e in v.link_edges)]; [x.select_set(False) for x in list(bm.verts) + list(bm.edges) + list(bm.faces)]; bm.select_history.clear(); S[0].select_set(True); bm.select_history.add(S[0]); bm.select_flush_mode(); bmesh.update_edit_mesh(me)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=2, location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[12])
bpy.context.scene.cursor.location = (0.1, 0.4, 0.0)
#@frap bpy.ops.mesh.rip_move(TRANSFORM_OT_translate={'value': (0.1, 0.3, 0.2)})
#@blender import bmesh; from mathutils import Vector; me = bpy.context.object.data; bm = bmesh.from_edit_mesh(me); bm.verts.ensure_lookup_table(); E = lambda a, b: next(e for e in bm.edges if {v.index for v in e.verts} == {a, b}); bmesh.utils.vert_separate(bm.verts[12], [E(12, 11), E(12, 13)]); bm.verts.index_update(); S = [v for v in bm.verts if v.co.length < 1e-6 and any(e.other_vert(v).index == 17 for e in v.link_edges)]; [x.select_set(False) for x in list(bm.verts) + list(bm.edges) + list(bm.faces)]; bm.select_history.clear(); S[0].select_set(True); bm.select_history.add(S[0]); bm.select_flush_mode(); bmesh.update_edit_mesh(me)
#@blender bpy.ops.transform.translate(value=(0.1, 0.3, 0.2))
