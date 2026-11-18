#!/usr/bin/env python3
"""Mimic a pi-style TUI: content above, an input box on the last row, cursor parked at the caret.
Usage: fake_tui.py hidden|visible"""
import os, sys, time
mode = sys.argv[1]
rows = os.get_terminal_size().lines
out = sys.stdout
out.write("\033[2J\033[H")
for i in range(1, rows - 1):
    out.write(f"conversation line {i}\r\n")
out.write("\033[7m status: model ready \033[0m\r\n")
out.write("> type here: MARKER\033[7m \033[0m")      # drawn (fake) reverse-video cursor
out.write(f"\033[{rows};20H")                       # park the real cursor at the caret
out.write("\033[?25l" if mode == "hidden" else "\033[?25h")
out.flush()
while True:
    time.sleep(1)
