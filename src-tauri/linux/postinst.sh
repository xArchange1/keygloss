#!/bin/sh
# Applies the udev rule to the running session: no reboot or new login needed.
modprobe uinput 2>/dev/null || true
udevadm control --reload-rules 2>/dev/null || true
udevadm trigger --subsystem-match=input --subsystem-match=misc 2>/dev/null || true
exit 0
