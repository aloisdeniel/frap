# WP 8.3.12: UV_PROJECT through projector objects: an empty (its inverse world matrix), a
# perspective camera (shifted, aspect and scale), an orthographic camera with a vertical sensor
# fit, a panoramic camera (`ProjCameraInfo`), three projectors picked per face by the face
# normal, a missing projector slot, a new UV map name and the `UVProjectModifier.projectors`
# binding (length `projector_count`, `projectors[i].object`).
# golden: startup=empty compare=evaluated:A,evaluated:B,evaluated:C,evaluated:D,evaluated:E,evaluated:F tier=T2
import bpy
bpy.ops.object.empty_add(location=(0.3, -2, 0.5), rotation=(1.2, 0.1, 0.3))
bpy.context.object.name = 'P1'
bpy.ops.object.empty_add(location=(2, 0.2, 0.1), rotation=(0.1, 1.4, 0))
bpy.context.object.name = 'P2'
bpy.ops.object.camera_add(location=(0.5, -5, 1.5), rotation=(1.3, 0, 0.1))
bpy.context.object.name = 'Persp'
bpy.data.objects['Persp'].data.shift_x = 0.1
bpy.data.objects['Persp'].data.lens = 35
bpy.ops.object.camera_add(location=(0, 0, 5), rotation=(0.1, 0, 0.2))
bpy.context.object.name = 'Ortho'
bpy.data.objects['Ortho'].data.type = 'ORTHO'
bpy.data.objects['Ortho'].data.ortho_scale = 3.5
bpy.data.objects['Ortho'].data.sensor_fit = 'VERTICAL'
bpy.ops.object.camera_add(location=(0.2, 0.1, 0.3), rotation=(1.5, 0, 0))
bpy.context.object.name = 'Pano'
bpy.data.objects['Pano'].data.type = 'PANO'
bpy.ops.mesh.primitive_uv_sphere_add(segments=16, ring_count=8)
bpy.context.object.name = 'A'
bpy.data.objects['A'].modifiers.new(name='M', type='UV_PROJECT')
bpy.data.objects['A'].modifiers['M'].projectors[0].object = bpy.data.objects['P1']
bpy.ops.mesh.primitive_uv_sphere_add(segments=16, ring_count=8)
bpy.context.object.name = 'B'
bpy.data.objects['B'].modifiers.new(name='M', type='UV_PROJECT')
bpy.data.objects['B'].modifiers['M'].projectors[0].object = bpy.data.objects['Persp']
bpy.data.objects['B'].modifiers['M'].aspect_x = 1.5
bpy.data.objects['B'].modifiers['M'].scale_y = 0.8
bpy.ops.mesh.primitive_uv_sphere_add(segments=16, ring_count=8, location=(0.2, 0.3, 0))
bpy.context.object.name = 'C'
bpy.data.objects['C'].modifiers.new(name='M', type='UV_PROJECT')
bpy.data.objects['C'].modifiers['M'].projectors[0].object = bpy.data.objects['Ortho']
bpy.data.objects['C'].modifiers['M'].aspect_y = 2
bpy.ops.mesh.primitive_uv_sphere_add(segments=16, ring_count=8)
bpy.context.object.name = 'D'
bpy.data.objects['D'].modifiers.new(name='M', type='UV_PROJECT')
bpy.data.objects['D'].modifiers['M'].projectors[0].object = bpy.data.objects['Pano']
bpy.data.objects['D'].modifiers['M'].scale_x = 2
bpy.ops.mesh.primitive_uv_sphere_add(segments=16, ring_count=8)
bpy.context.object.name = 'E'
bpy.data.objects['E'].modifiers.new(name='M', type='UV_PROJECT')
bpy.data.objects['E'].modifiers['M'].projector_count = 4
bpy.data.objects['E'].modifiers['M'].projectors[0].object = bpy.data.objects['P1']
bpy.data.objects['E'].modifiers['M'].projectors[1].object = bpy.data.objects['P2']
bpy.data.objects['E'].modifiers['M'].projectors[3].object = bpy.data.objects['Ortho']
bpy.data.objects['E'].modifiers['M'].projectors[2].object
bpy.data.objects['E'].modifiers['M'].projectors[4].object = bpy.data.objects['P1']
bpy.ops.mesh.primitive_cube_add(size=1.5)
bpy.context.object.name = 'F'
bpy.data.objects['F'].rotation_euler = (0.2, 0.3, 0.1)
bpy.data.objects['F'].modifiers.new(name='M', type='UV_PROJECT')
bpy.data.objects['F'].modifiers['M'].uv_layer = 'Missing'
bpy.data.objects['F'].modifiers['M'].projectors[0].object = bpy.data.objects['P2']
bpy.data.objects['F'].modifiers['M'].projectors[0].object = bpy.data.objects['F']
bpy.data.meshes['Cube'].uv_layers.new(name='Second')
bpy.data.objects['F'].modifiers['M'].uv_layer = 'Second'
