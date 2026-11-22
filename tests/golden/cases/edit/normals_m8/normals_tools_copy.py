# WP 8.3.2: mesh.normals_tools COPY priorities and multi-object editing: the only selected
# custom normal (a plane corner; on the cube, whose corners point at a target, the corner
# picked by selection history: vertex then face in vertex+face select mode, which wins over
# the face normal), a second mesh in Edit Mode with a selection reports the error after the
# first copied, PASTE and ADD act on both meshes, and a mesh without corners (edges only) is
# skipped. The buffer is recorded in object locations.
# golden: startup=factory compare=objects,context,mesh:Plane,mesh:Cube,mesh:Loose tier=T1
import bpy
bpy.ops.mesh.primitive_plane_add(size=2, location=(4, 0, 0), rotation=(0.3, 0.2, 0.0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[2])
bpy.ops.mesh.normals_tools(mode='COPY')
bpy.data.objects['Camera'].location = bpy.context.scene.tool_settings.normal_vector
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.point_normals(target_location=(0.3, 0.5, 4.0))
bpy.ops.mesh.select_mode(type='FACE', use_extend=True)
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.frap.select_elements(type='FACE', indices=[0], action='ADD')
bpy.ops.mesh.normals_tools(mode='COPY')
bpy.data.objects['Light'].location = bpy.context.scene.tool_settings.normal_vector
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Plane'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[1])
bpy.ops.frap.select_elements(type='FACE', indices=[0], action='ADD')
bpy.ops.frap.select_elements(type='VERT', indices=[3], object='Plane')
bpy.ops.mesh.normals_tools(mode='COPY')
bpy.context.scene.tool_settings.normal_vector = (0.0, 1.0, 1.0)
bpy.ops.mesh.normals_tools(mode='PASTE')
bpy.ops.mesh.select_all(action='SELECT')
bpy.context.scene.tool_settings.normal_vector = (1.0, 0.0, 0.0)
bpy.ops.mesh.normals_tools(mode='ADD')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 4, 0))
bpy.data.objects['Plane.001'].name = 'Loose'
bpy.data.meshes['Plane.001'].name = 'Loose'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.delete(type='ONLY_FACE')
bpy.ops.mesh.normals_tools(mode='COPY')
bpy.ops.mesh.normals_tools(mode='RESET')
