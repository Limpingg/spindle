#!/usr/bin/env python3
"""Settles the version conflict a Spotifast merge leaves in Cargo.toml.

    scripts/settle-version.py Cargo.toml <spindle version>

A conflict hunk whose lines are all `version = "..."` becomes the Spindle
version; any other hunk is left as it is, for a person. The package's own
version line (the first in the file) is then set to the Spindle version,
which also covers a merge where that line did not conflict.
"""

import re
import sys

path, version = sys.argv[1:3]
text = open(path).read()
hunk = re.compile(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", re.S)


def settle(match: re.Match) -> str:
    sides = match.group(1).splitlines() + match.group(2).splitlines()
    if sides and all(re.fullmatch(r'version = "[^"]*"', line) for line in sides):
        return f'version = "{version}"\n'
    return match.group(0)


text = hunk.sub(settle, text)
text = re.sub(r'(?m)^version = "[^"]*"', f'version = "{version}"', text, count=1)
open(path, "w").write(text)
