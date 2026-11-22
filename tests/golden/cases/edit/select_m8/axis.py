# WP 8.3.1: mesh.select_axis from the active vertex of a rotated, parented ico sphere: every
# orientation (GLOBAL, LOCAL, NORMAL, GIMBAL with Euler and axis-angle rotation, VIEW, CURSOR,
# PARENT), every sign (POS, NEG, ALIGN) and axis, a threshold, a second mesh in Edit Mode, and
# the WARNING without an active vertex (CANCELLED). Intermediate selections are kept as renamed
# attributes.
# golden: startup=factory compare=mesh:Icosphere,mesh:Cube tier=T1
import bpy
bpy.ops.object.empty_add(location=(0, 0, 0), rotation=(0.4, 0, 0))
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, location=(1, 0, 0), rotation=(0.2, 0.3, 0.5))
bpy.data.objects['Icosphere'].parent = bpy.data.objects['Empty']
bpy.context.scene.cursor.rotation_euler = (0.0, 0.6, 0.0)
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Icosphere']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_axis()
bpy.ops.frap.select_elements(type='VERT', indices=[30], object='Icosphere')
bpy.ops.mesh.select_axis(orientation='GLOBAL', axis='Z')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'global_z'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[30], object='Icosphere')
bpy.ops.mesh.select_axis(axis='Y', sign='NEG')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'local_y_neg'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[30], object='Icosphere')
bpy.ops.mesh.select_axis(orientation='NORMAL', axis='Z', sign='ALIGN', threshold=0.3)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'normal_align'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[40], object='Icosphere')
bpy.ops.mesh.select_axis(orientation='NORMAL', axis='X')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'normal_x'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[30], object='Icosphere')
bpy.ops.mesh.select_axis(orientation='GIMBAL', axis='X')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'gimbal_euler'
bpy.data.objects['Icosphere'].rotation_mode = 'AXIS_ANGLE'
bpy.data.objects['Icosphere'].rotation_axis_angle = (0.8, 0.0, 1.0, 1.0)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[30], object='Icosphere')
bpy.ops.mesh.select_axis(orientation='GIMBAL', axis='Y', sign='NEG')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'gimbal_axis_angle'
bpy.data.objects['Icosphere'].rotation_mode = 'QUATERNION'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[30], object='Icosphere')
bpy.ops.mesh.select_axis(orientation='GIMBAL', axis='Z')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'gimbal_quat'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[35], object='Icosphere')
bpy.ops.mesh.select_axis(orientation='PARENT', axis='Y')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'parent_y'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[35], object='Icosphere')
bpy.ops.mesh.select_axis(orientation='VIEW', axis='X', sign='NEG')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'view_x'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[35], object='Icosphere')
bpy.ops.mesh.select_axis(orientation='CURSOR', axis='X')
