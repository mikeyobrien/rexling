#!/usr/bin/env python3
"""Compact color previews for reviewing the two concepts in Rex terminals."""
import sys

A = sys.argv[1:] == ['a']
W = 35
FG = {'base': '233;239;245', 'muted': '163;177;190', 'mint': '165;236;201'}
BG = {'phone': '20;24;30', 'sheet': '34;42;51', 'keys': '43;53;65', 'pop': '41;71;57'}


def paint(s, fg='base', bg=None):
    return f'\x1b[38;2;{FG[fg]}m' + (f'\x1b[48;2;{BG[bg]}m' if bg else '') + s + '\x1b[0m'


def row(s='', fg='base', bg='phone'):
    print('│' + paint(s.ljust(W), fg, bg) + '│')


print(paint('A  Bottom sheet  ·  recommended' if A else 'B  Above-keyboard tray', 'mint'))
print('╭' + '─' * W + '╮')
row('  9:41                         89%', 'muted')
row('  ▾ rexling                       ⋮')
row('  ───────────────────────────────', 'muted')
row('  mikey@reef ~/Projects/rexling', 'mint')
row('  $ git status --short', 'muted')
row('   M app/src/main/TerminalChrome.kt', 'muted')
row('  $ ▌', 'mint')
row()
row('                 ━━━━━' if A else '', 'muted')
row('  Commands · Shell               ×', 'base', 'sheet')
if A:
    row('  ⌕  Search commands', 'muted', 'keys')
for label in ('◫  Split pane…', '⛶  Zoom pane', '▤  Copy viewport'):
    row('  ' + label, 'base', 'sheet')
if A:
    row('  ✎  Rename tab…', 'base', 'sheet')
    row('  ⇄  Port forwarding…', 'base', 'sheet')
    row('', 'base', 'sheet')
else:
    row('  ⌕  More commands…', 'mint', 'sheet')
    print('│' + paint(' Esc Ctrl Alt Shift Tab  ↔   ', 'base', 'keys') + paint('  ⌘   ', 'mint', 'pop') + '│')
    for line in (' q  w  e  r  t  y  u  i  o  p', '   a  s  d  f  g  h  j  k  l', '     z  x  c  v  b  n  m', ' ?123        English          ↵'):
        row(line, 'muted', 'keys')
row('             ━━━━━━━━━', 'muted')
print('╰' + '─' * W + '╯')
if A:
    print('Tap ⌘ → keyboard hides; search is\noptional. Back returns to the terminal.')
else:
    print('Tap ⌘ → keyboard stays; three commands.\nMore commands opens the full sheet.')
