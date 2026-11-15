#!/bin/sh
# Takes the input device access away again.
udevadm control --reload-rules 2>/dev/null || true
udevadm trigger --subsystem-match=input --subsystem-match=misc 2>/dev/null || true
exit 0
