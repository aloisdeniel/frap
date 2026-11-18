# WP 2.12: the crease and bevel-weight layers are FLOAT layers looked up by name and type.
# A same-named attribute of another domain is dropped when the vertex (or edge) layer is
# written (a UV map included); an edge layer named like a vertex attribute is lost. (A
# same-named layer of another type is covered by unit tests: Blender's edge path is undefined
# behaviour there, and frap rolls back the cancelled vertex path.)
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T1 tol=1e-5
import bpy
bpy.data.meshes['Cube'].attributes.new(name='crease_vert', type='FLOAT', domain='EDGE')
bpy.data.meshes['Cube'].attributes.new(name='bevel_weight_edge', type='FLOAT', domain='POINT')
bpy.data.meshes['Cube'].attributes.new(name='crease_edge', type='FLOAT', domain='FACE')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.vert_crease(value=0.5)
bpy.ops.transform.edge_bevelweight(value=0.5)
bpy.ops.transform.edge_crease(value=0.5)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.data.meshes['Cube.001'].uv_layers.new(name='crease_edge')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.transform.edge_crease(value=0.25)
bpy.ops.object.mode_set(mode='OBJECT')
