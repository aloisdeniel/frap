# WP 8.3.2: mesh.normals_tools in every mode on a smooth Suzanne with sharp edges. The copy
# buffer is scene.tool_settings.normal_vector; it is written into the locations and scales of
# the factory objects so the dump compares it. COPY takes a smooth vertex's common normal, a
# sharp vertex (normals differ: nothing copied) and a face normal, and reports an error for
# several vertices; PASTE normalises the buffer (and pastes nothing when it is zero);
# absolute PASTE points the normals at the buffer as a position; ADD, MULTIPLY; RESET zeroes
# the buffer and the selected custom normals.
# golden: startup=factory compare=objects,context,mesh:Suzanne tier=T1
import bpy
bpy.ops.mesh.normals_tools(mode='COPY')
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.shade_smooth()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.set_sharpness_by_angle(angle=0.8)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.normals_tools(mode='COPY')
bpy.ops.mesh.normals_tools(mode='PASTE')
bpy.ops.frap.select_elements(type='VERT', indices=[16])
bpy.ops.mesh.normals_tools(mode='COPY')
bpy.data.objects['Camera'].location = bpy.context.scene.tool_settings.normal_vector
bpy.ops.frap.select_elements(type='VERT', indices=[20, 21, 22, 23, 40])
bpy.ops.mesh.normals_tools(mode='COPY')
bpy.ops.mesh.normals_tools(mode='PASTE')
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.mesh.normals_tools(mode='COPY')
bpy.data.objects['Light'].location = bpy.context.scene.tool_settings.normal_vector
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[100])
bpy.ops.mesh.normals_tools(mode='COPY')
bpy.data.objects['Cube'].location = bpy.context.scene.tool_settings.normal_vector
bpy.ops.frap.select_elements(type='FACE', indices=[150, 151, 152, 153])
bpy.ops.mesh.normals_tools(mode='PASTE')
bpy.context.scene.tool_settings.normal_vector = (3.0, -4.0, 12.0)
bpy.ops.frap.select_elements(type='FACE', indices=[200, 201])
bpy.ops.mesh.normals_tools(mode='PASTE')
bpy.data.objects['Camera'].scale = bpy.context.scene.tool_settings.normal_vector
bpy.context.scene.tool_settings.normal_vector = (0.5, 2.0, 1.0)
bpy.ops.frap.select_elements(type='FACE', indices=[300, 301, 302])
bpy.ops.mesh.normals_tools(mode='PASTE', absolute=True)
bpy.context.scene.tool_settings.normal_vector = (0.0, 0.5, -0.25)
bpy.ops.frap.select_elements(type='FACE', indices=[10, 11, 12])
bpy.ops.mesh.normals_tools(mode='ADD')
bpy.context.scene.tool_settings.normal_vector = (2.0, 0.25, 1.0)
bpy.ops.frap.select_elements(type='FACE', indices=[400, 401, 402, 403])
bpy.ops.mesh.normals_tools(mode='MULTIPLY')
bpy.context.scene.tool_settings.normal_vector = (0.0, 0.0, 0.0)
bpy.ops.frap.select_elements(type='FACE', indices=[450])
bpy.ops.mesh.normals_tools(mode='MULTIPLY')
bpy.ops.mesh.normals_tools(mode='PASTE')
bpy.context.scene.tool_settings.normal_vector = (1.0, 1.0, 1.0)
bpy.ops.frap.select_elements(type='FACE', indices=[200, 450])
bpy.ops.mesh.normals_tools(mode='RESET')
bpy.data.objects['Light'].scale = bpy.context.scene.tool_settings.normal_vector
