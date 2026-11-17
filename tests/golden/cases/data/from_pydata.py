# WP 1.22: `Mesh.from_pydata` (attribute order, derived edges, `shade_flat`), `Mesh.shade_smooth`
# / `shade_flat`, and the errors of these Python-defined methods. Blender leaves a mesh partly
# filled when `from_pydata` fails after its first step (frap rolls the statement back), so each
# such line uses a throw-away mesh created by the line before. `Quad` has three faces: the frap
# dumper's bulk shortcut misreads `polygons/vertices` of one or two quads (open issue).
# golden: startup=factory tier=T0 compare=objects,mesh:Quad,mesh:Wire,mesh:Mixed,mesh:Smooth
import bpy
bpy.data.meshes.new('Quad').from_pydata([(0, 0, 0), (1, 0, 0), (2, 0, 0), (3, 0, 0), (0, 1, 0), (1, 1, 0), (2, 1, 0), (3, 1, 0)], [], [(0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6)])
bpy.data.meshes.new('Wire').from_pydata([(0, 0, 0), (1, 0, 0), (1, 1, 1)], [(0, 1), (1, 2)], [])
bpy.data.meshes.new('Mixed').from_pydata([[0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 0, 1]], [(0, 1)], [[0, 1, 2], (0, 1, 3)])
bpy.data.meshes.new('Smooth').from_pydata(((0, 0, 0), (2, 0, 0), (0, 2, 0)), (), ((0, 1, 2),), shade_flat=False)
bpy.data.meshes['Smooth'].shade_flat()
bpy.data.meshes['Smooth'].shade_smooth()
bpy.data.meshes['Quad'].shade_smooth()
bpy.data.meshes['Quad'].shade_flat()
bpy.data.meshes['Wire'].from_pydata([], [], [], shade_flat=False)
bpy.data.meshes.new('Empty').from_pydata([], [], [])
bpy.data.meshes.new('B1')
bpy.data.meshes['B1'].from_pydata([(0, 0), (1, 0), (1, 1)], [], [(0, 1, 2)])
bpy.data.meshes.new('B2')
bpy.data.meshes['B2'].from_pydata([(0, 0, 'a')], [], [])
bpy.data.meshes.new('B3')
bpy.data.meshes['B3'].from_pydata([(0, 0, 0), (1, 0, 0), (1, 1, 0)], [], [(0, 1, 2.5)])
bpy.data.meshes.new('B4').from_pydata([(0, 0, 0)], [], [])
bpy.data.meshes['B4'].from_pydata([(1, 0, 0)], [], [])
bpy.data.meshes.new('Args')
bpy.data.meshes['Args'].from_pydata(5, [], [])
bpy.data.meshes['Args'].from_pydata([(0, 0, 0)], [])
bpy.data.meshes['Args'].from_pydata()
bpy.data.meshes['Args'].from_pydata([(0, 0, 0)], [], [], foo=1)
bpy.data.meshes['Args'].from_pydata([(0, 0, 0)], [], [], True, 2)
bpy.data.meshes['Args'].from_pydata([(0, 0, 0)], [], [], vertices=[])
bpy.data.meshes['Args'].shade_smooth(1)
bpy.data.meshes['Args'].shade_flat(x=1)
bpy.data.meshes['Args'].from_pydata(vertices=[(0, 0, 0)], edges=[], faces=[])
bpy.context.scene.collection.objects.link(bpy.data.objects.new('Quad', bpy.data.meshes['Quad']))
bpy.data.objects['Quad'].location = (0, 0, 2)
