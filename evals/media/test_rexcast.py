#!/usr/bin/env python3
"""Unit tests for scripts/rexcast: input parsing, pacing and tile selection. Run: python3 evals/media/test_rexcast.py"""
import importlib.machinery
import importlib.util
import os
import sys
import unittest

from PIL import Image

sys.dont_write_bytecode = True  # loading scripts/rexcast must not leave a cache beside it

PATH = os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../scripts/rexcast")
loader = importlib.machinery.SourceFileLoader("rexcast", PATH)
rexcast = importlib.util.module_from_spec(importlib.util.spec_from_loader("rexcast", loader))
loader.exec_module(rexcast)


class FakePane:
    cols, rows, cw, ch = 24, 12, 10, 20

    def __init__(self):
        self.out = []

    def write(self, text):
        self.out.append(text)


class InputTest(unittest.TestCase):
    def test_mouse_keys_and_answers_are_separated_across_split_reads(self):
        parser = rexcast.Input()
        events = list(parser.feed(b"ab\x1b[<0;12;34M\x1b_rexcast;ac"))
        self.assertEqual(events, [("key", b"ab"), ("mouse", (0, 12, 34, True))])
        self.assertEqual(list(parser.feed(b"k=7\x1b\\\x1b[<0;12;3")), [("ack", 7)])
        self.assertEqual(list(parser.feed(b"4m\x1b")), [("mouse", (0, 12, 34, False)), ("key", b"\x1b")])

    def test_an_answer_that_is_not_a_number_is_dropped(self):
        self.assertEqual(list(rexcast.Input().feed(b"\x1b_rexcast;ack=rm -rf\x1b\\x")), [("key", b"x")])


class FlowTest(unittest.TestCase):
    def test_one_frame_flies_until_the_link_answers_quickly_then_a_window(self):
        pane, tiles = FakePane(), rexcast.Tiles(FakePane(), 0, 0, 24, 12, 240, 240, lossy=True, codec="auto")
        flow = rexcast.Flow(True, 30)
        self.assertTrue(flow.ready(0.0))
        flow.mark(pane, 0.0)
        self.assertIn("rexcast;frame=1", pane.out[-1])
        self.assertFalse(flow.ready(0.01))
        flow.answer(1, 0.03, tiles)
        self.assertEqual(flow.level, 3)  # a quick first answer earns a bigger picture, in steps
        for serial in range(rexcast.Flow.WINDOW):
            self.assertTrue(flow.ready(0.04))
            flow.mark(pane, 0.04)
        self.assertFalse(flow.ready(0.05))

    def test_a_silent_viewer_gets_smaller_frames_less_and_less_often(self):
        pane, flow = FakePane(), rexcast.Flow(True, 30)
        flow.last_sent = flow.last_ack = 0.0
        flow.level = level = 2
        flow.mark(pane, 0.0)
        self.assertFalse(flow.ready(0.9))
        self.assertTrue(flow.ready(1.1))
        self.assertEqual((flow.level, flow.silence), (level + 1, 2.0))

    def test_a_viewer_that_does_not_answer_is_never_paced(self):
        flow = rexcast.Flow(False, 30)
        flow.mark(FakePane(), 0.0)
        self.assertEqual(flow.serial, 0)
        self.assertTrue(flow.ready(0.0))


class TilesTest(unittest.TestCase):
    def frame(self, colour):
        return Image.new("RGB", (240, 240), colour)

    def test_motion_is_one_jpeg_under_the_grid_until_exact_tiles_return(self):
        pane = FakePane()
        tiles = rexcast.Tiles(pane, 0, 0, 24, 12, 240, 240, tile=(12, 6), codec="auto", lossy=True)
        self.assertEqual(tiles.draw(self.frame((10, 20, 30))), 4)
        sent = "".join(pane.out)
        self.assertEqual(sent.count("a=T"), 1)  # every tile changed: one picture, underneath
        self.assertIn("i=3999,c=24,r=12,z=-1,f=100", sent)
        self.assertEqual(sent.count("a=d"), 4)  # the tiles it stands in for are taken away
        self.assertEqual(len(tiles.rough), 4)
        self.assertEqual(tiles.draw(self.frame((10, 20, 30))), 0)
        changed = self.frame((10, 20, 30))
        changed.putpixel((5, 5), (255, 255, 255))
        del pane.out[:]
        self.assertEqual(tiles.draw(changed), 1)  # one tile of four: sent exact, on top
        self.assertIn("i=4000,c=12,r=6,f=24,o=z", "".join(pane.out))
        self.assertEqual(len(tiles.rough), 3)
        tiles.hurry()
        self.assertEqual(tiles.refine(), 1)  # a few per pass
        self.assertTrue(tiles.covered)
        while tiles.refine():
            pass
        self.assertEqual((tiles.rough, tiles.covered), ({}, False))
        self.assertIn("a=d,d=I,i=3999", pane.out[-1])  # the JPEG goes with the last exact tile

    def test_motion_in_part_of_the_picture_covers_only_that_part(self):
        pane = FakePane()
        tiles = rexcast.Tiles(pane, 0, 0, 24, 12, 240, 240, tile=(6, 6), codec="auto", lossy=True)  # 4 by 2 tiles
        frame = self.frame((0, 0, 0))
        tiles.draw(frame)
        tiles.hurry()
        while tiles.refine():
            pass
        frame.paste((200, 0, 0), (0, 0, 180, 120))  # the top row's first three tiles
        del pane.out[:]
        self.assertEqual(tiles.draw(frame), 3)
        self.assertIn("i=3999,c=18,r=6,z=-1", "".join(pane.out))

    def test_standard_terminals_get_exact_pixels_only(self):
        pane = FakePane()
        tiles = rexcast.Tiles(pane, 0, 0, 24, 12, 240, 240, codec="auto", lossy=False)
        tiles.draw(self.frame((1, 2, 3)))
        self.assertEqual(tiles.rough, {})
        self.assertTrue(all("f=24,o=z" in text for text in pane.out if "_G" in text and "a=T" in text))


if __name__ == "__main__":
    unittest.main()
