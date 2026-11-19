# WP 3.9: mirrored and scaled placements. A wedge (a cone with 3 vertices) is placed four times
# sharing one mesh: plain, mirrored on X (negative scale: the exporter keeps the node scale
# negative, as Blender does), non-uniformly scaled and rotated in YXZ Euler order, and one
# copy has its scale applied (object.transform_apply, so its own mesh with baked positions).
# Round trip (§17.2).
# golden: startup=factory tier=T2 compare=glb:out/wedges.glb,glb:out/wedges_rt.glb
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_cone_add(vertices=3, radius1=1, depth=1.5, location=(0, 0, 0))
bpy.context.object.name = 'Wedge'
bpy.data.objects['Wedge'].data.materials.append(bpy.data.materials['Material'])
bpy.data.objects.new('WedgeMirror', bpy.data.objects['Wedge'].data)
bpy.context.scene.collection.objects.link(bpy.data.objects['WedgeMirror'])
bpy.data.objects['WedgeMirror'].location = (3, 0, 0)
bpy.data.objects['WedgeMirror'].scale = (-1, 1, 1)
bpy.data.objects.new('WedgeStretch', bpy.data.objects['Wedge'].data)
bpy.context.scene.collection.objects.link(bpy.data.objects['WedgeStretch'])
bpy.data.objects['WedgeStretch'].location = (0, 3, 0)
bpy.data.objects['WedgeStretch'].scale = (1, 2, 0.5)
bpy.data.objects['WedgeStretch'].rotation_mode = 'YXZ'
bpy.data.objects['WedgeStretch'].rotation_euler = (0.3, 0.6, 0.9)
bpy.ops.object.duplicate_move(TRANSFORM_OT_translate={'value': (-3, 0, 0)})
bpy.context.object.scale = (1.5, 1.5, 3)
bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
bpy.ops.export_scene.gltf(filepath='out/wedges.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/wedges.glb')
bpy.ops.export_scene.gltf(filepath='out/wedges_rt.glb')
