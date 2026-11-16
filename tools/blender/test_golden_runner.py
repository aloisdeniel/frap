# SPDX-License-Identifier: GPL-3.0-or-later
"""Blender-free unit tests of golden_runner.py's time tracking (WP 1.8, §20.3 / D-04).

    python3 -I tools/blender/test_golden_runner.py

Run by the Rust test `golden_runner_python_unit_tests` when python3 is available.
"""

import os
import sys
import unittest

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import golden_runner as gr  # noqa: E402


class TimeDirty(unittest.TestCase):
    def after(self, text, dirty=False, changed=False):
        return gr.time_dirty_after(dirty, text, changed)

    def test_writes_make_time_dirty(self):
        for text in [
            "bpy.context.scene.frame_current = 6",
            "scene.frame_current += 5",
            "scene.frame_current -= 1",
            "scene.frame_current *= 2",
            "scene.frame_current //= 2",
            "scene.frame_float = 6.5",
            "scene.frame_subframe = 0.25",
            "setattr(scene, 'frame_current', 6)",
            'setattr(bpy.context.scene, "frame_float", 6.0)',
            "a = 1; scene.frame_current += 1",
        ]:
            self.assertTrue(self.after(text), text)
            self.assertTrue(self.after(text, changed=True), text)

    def test_evaluations_clear_it(self):
        for text in [
            "bpy.context.view_layer.update()",
            "bpy.context.scene.frame_set(6)",
            "dg = bpy.context.evaluated_depsgraph_get()",
            "bpy.ops.object.select_all(action='SELECT')",
            "scene.frame_current += 5; bpy.context.view_layer.update()",
        ]:
            self.assertFalse(self.after(text, dirty=True), text)
        # frame_set and the frame operators change the frame and evaluate it (Blender 5.2.4:
        # screen.frame_offset applies the new frame's animation before returning).
        self.assertFalse(self.after("scene.frame_set(4)", dirty=True, changed=True))
        self.assertFalse(self.after("bpy.ops.screen.frame_offset(delta=5)", True, True))

    def test_the_last_token_decides(self):
        self.assertTrue(self.after("bpy.context.view_layer.update(); scene.frame_current += 1"))
        self.assertFalse(self.after("scene.frame_current = 2; bpy.ops.object.select_all()", True))

    def test_reads_and_comparisons_are_not_writes(self):
        for text in [
            "x = scene.frame_current",
            "scene.frame_current == 5",
            "assert scene.frame_current != 5",
            "scene.frame_current <= 5",
            "scene.frame_current >= 5",
            "bpy.data.objects['Cube'].location = (1, 2, 3)",
        ]:
            self.assertFalse(self.after(text), text)
            self.assertTrue(self.after(text, dirty=True), text)

    def test_unseen_frame_changes_are_caught_by_value(self):
        # A frame change no pattern sees (helper, computed name) makes time dirty.
        for text in [
            "move_time(scene)",
            "setattr(scene, name, 6)",
            "for f in (2, 3): scene.__setattr__('frame_' + 'current', f)",
        ]:
            self.assertTrue(self.after(text, changed=True), text)
            self.assertFalse(self.after(text, changed=False), text)

    def test_frames_differ(self):
        self.assertFalse(gr.frames_differ({"Scene": (1, 0.0)}, {"Scene": (1, 0.0)}))
        self.assertTrue(gr.frames_differ({"Scene": (1, 0.0)}, {"Scene": (6, 0.0)}))
        self.assertTrue(gr.frames_differ({"Scene": (1, 0.0)}, {"Scene": (1, 0.5)}))
        # Scenes added or removed by the statement are not time changes.
        self.assertFalse(gr.frames_differ({"Scene": (1, 0.0)}, {"Scene": (1, 0.0), "S2": (9, 0.0)}))
        self.assertFalse(gr.frames_differ({"Gone": (1, 0.0)}, {}))
        self.assertFalse(gr.frames_differ(None, {"Scene": (1, 0.0)}))


if __name__ == "__main__":
    unittest.main()
