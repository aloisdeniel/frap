# M5+M6 stabilization round 7: Blender resyncs layer collections lazily (`VIEW_LAYER_OUT_OF_SYNC`,
# `BKE_view_layer_synced_ensure`), from the tree of the previous sync (`layer_collection_resync_
# find`). A collection unlinked and linked again keeps its layer collection's flags; with a
# resync in between (view_layer.update(), an operator) it gets its parent's flags instead; a
# collection moved (linked under P, then unlinked from A) keeps them; a collection linked under
# an excluded, hidden, holdout parent takes the parent's flags (`child_layer->flag =
# parent_layer_flag`) and including the parent includes it. Each probe's location holds
# (exclude, holdout, hide_viewport) of the layer collection after a step.
# golden: startup=factory compare=objects tier=T1 tol=1e-6
import bpy
bpy.data.collections.new('A')
bpy.context.scene.collection.children.link(bpy.data.collections['A'])
bpy.data.collections.new('B')
bpy.data.collections['A'].children.link(bpy.data.collections['B'])
bpy.context.view_layer.layer_collection.children['A'].children['B'].exclude = True
bpy.context.view_layer.layer_collection.children['A'].children['B'].holdout = True
bpy.data.collections['A'].children.unlink(bpy.data.collections['B'])
bpy.data.collections['A'].children.link(bpy.data.collections['B'])
bpy.data.objects.new('P_Relinked', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Relinked'])
bpy.data.objects['P_Relinked'].location = (bpy.context.view_layer.layer_collection.children['A'].children['B'].exclude, bpy.context.view_layer.layer_collection.children['A'].children['B'].holdout, bpy.context.view_layer.layer_collection.children['A'].children['B'].hide_viewport)
bpy.data.collections['A'].children.unlink(bpy.data.collections['B'])
bpy.context.view_layer.update()
bpy.data.collections['A'].children.link(bpy.data.collections['B'])
bpy.data.objects.new('P_Updated', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Updated'])
bpy.data.objects['P_Updated'].location = (bpy.context.view_layer.layer_collection.children['A'].children['B'].exclude, bpy.context.view_layer.layer_collection.children['A'].children['B'].holdout, bpy.context.view_layer.layer_collection.children['A'].children['B'].hide_viewport)
bpy.context.view_layer.layer_collection.children['A'].children['B'].exclude = True
bpy.data.collections['A'].children.unlink(bpy.data.collections['B'])
bpy.ops.object.select_all(action='DESELECT')
bpy.data.collections['A'].children.link(bpy.data.collections['B'])
bpy.data.objects.new('P_Operator', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Operator'])
bpy.data.objects['P_Operator'].location = (bpy.context.view_layer.layer_collection.children['A'].children['B'].exclude, bpy.context.view_layer.layer_collection.children['A'].children['B'].holdout, bpy.context.view_layer.layer_collection.children['A'].children['B'].hide_viewport)
bpy.context.view_layer.layer_collection.children['A'].children['B'].exclude = True
bpy.context.view_layer.layer_collection.children['A'].children['B'].hide_viewport = True
bpy.data.collections.new('P')
bpy.context.scene.collection.children.link(bpy.data.collections['P'])
bpy.data.collections['P'].children.link(bpy.data.collections['B'])
bpy.data.collections['A'].children.unlink(bpy.data.collections['B'])
bpy.data.objects.new('P_Moved', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Moved'])
bpy.data.objects['P_Moved'].location = (bpy.context.view_layer.layer_collection.children['P'].children['B'].exclude, bpy.context.view_layer.layer_collection.children['P'].children['B'].holdout, bpy.context.view_layer.layer_collection.children['P'].children['B'].hide_viewport)
bpy.context.view_layer.layer_collection.children['A'].exclude = True
bpy.context.view_layer.layer_collection.children['A'].holdout = True
bpy.context.view_layer.layer_collection.children['A'].hide_viewport = True
bpy.data.collections.new('N')
bpy.data.collections['A'].children.link(bpy.data.collections['N'])
bpy.data.objects.new('P_NewChild', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_NewChild'])
bpy.data.objects['P_NewChild'].location = (bpy.context.view_layer.layer_collection.children['A'].children['N'].exclude, bpy.context.view_layer.layer_collection.children['A'].children['N'].holdout, bpy.context.view_layer.layer_collection.children['A'].children['N'].hide_viewport)
bpy.context.view_layer.layer_collection.children['A'].exclude = False
bpy.data.objects.new('P_Included', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Included'])
bpy.data.objects['P_Included'].location = (bpy.context.view_layer.layer_collection.children['A'].children['N'].exclude, bpy.context.view_layer.layer_collection.children['A'].children['N'].holdout, bpy.context.view_layer.layer_collection.children['A'].children['N'].hide_viewport)
bpy.data.collections.new('M')
bpy.data.collections['P'].children.link(bpy.data.collections['M'])
bpy.data.collections['P'].children.unlink(bpy.data.collections['B'])
bpy.data.collections['M'].children.link(bpy.data.collections['B'])
bpy.data.objects.new('P_Deeper', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Deeper'])
bpy.data.objects['P_Deeper'].location = (bpy.context.view_layer.layer_collection.children['P'].children['M'].children['B'].exclude, bpy.context.view_layer.layer_collection.children['P'].children['M'].children['B'].holdout, bpy.context.view_layer.layer_collection.children['P'].children['M'].children['B'].hide_viewport)
