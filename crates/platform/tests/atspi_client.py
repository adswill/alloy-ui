#!/usr/bin/python3.12
"""Screen-reader side of the AT-SPI end-to-end test (run by tests/atspi.rs).

Acts like a minimal assistive technology: finds the application on the
accessibility bus by name, walks its tree, performs actions, then listens for
events while the test changes the tree. Prints one JSON document on stdout.
"""
import json
import sys

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi, GLib  # noqa: E402

app_name = sys.argv[1]
Atspi.init()


def find_app():
    desktop = Atspi.get_desktop(0)
    for i in range(desktop.get_child_count()):
        a = desktop.get_child_at_index(i)
        if a is not None and a.get_name() == app_name:
            return a
    return None


def snapshot(node):
    states = node.get_state_set()
    names = [s.value_nick for s in states.get_states()]
    d = {
        "name": node.get_name(),
        "role": node.get_role_name(),
        "states": names,
        "children": [snapshot(node.get_child_at_index(i)) for i in range(node.get_child_count())],
    }
    ext = node.get_extents(Atspi.CoordType.SCREEN) if hasattr(node, "get_extents") else None
    if ext is not None:
        d["extents"] = [ext.x, ext.y, ext.width, ext.height]
    return d


import time

app = None
for _ in range(100):
    app = find_app()
    if app is not None:
        break
    time.sleep(0.1)
if app is None:
    print(json.dumps({"error": "application not found"}))
    sys.exit(1)

out = {"app": snapshot(app)}
win = app.get_child_at_index(0)
by_name = {win.get_child_at_index(i).get_name(): win.get_child_at_index(i)
           for i in range(win.get_child_count())}

# Interfaces and values.
save = by_name["Save"]
out["save_interfaces"] = save.get_interfaces()
out["save_action_name"] = save.get_action_name(0)
out["name_text"] = Atspi.Text.get_text(by_name["Name"], 0, -1)
vol = by_name["Volume"]
out["volume"] = [Atspi.Value.get_minimum_value(vol), Atspi.Value.get_maximum_value(vol),
                 Atspi.Value.get_current_value(vol)]
# Hit test at the centre of the Save button.
e = save.get_extents(Atspi.CoordType.SCREEN)
hit = win.get_accessible_at_point if False else None
comp = Atspi.Component
hit = comp.get_accessible_at_point(win, e.x + e.width // 2, e.y + e.height // 2, Atspi.CoordType.SCREEN)
out["hit"] = hit.get_name() if hit else None

# Actions that must reach the app.
out["do_action"] = save.do_action(0)
out["set_text"] = Atspi.EditableText.set_text_contents(by_name["Name"], "Grace")
out["set_value"] = Atspi.Value.set_current_value(vol, 60.0)
out["grab_focus"] = Atspi.Component.grab_focus(by_name["Dark mode"])

# Listen for events while the test mutates the tree.
events = []


def on_event(ev):
    events.append({"type": ev.type, "source": ev.source.get_name() if ev.source else None,
                   "d1": ev.detail1})


listener = Atspi.EventListener.new(on_event)
for t in ("object:state-changed:focused", "object:property-change:accessible-name",
          "object:children-changed:add", "object:children-changed:remove",
          "object:text-changed:insert"):
    listener.register(t)
loop = GLib.MainLoop()
GLib.timeout_add(500, lambda: (print("READY", flush=True), False)[1])


def wait_go():
    sys.stdin.readline()
    GLib.timeout_add(1500, loop.quit)
    return False


GLib.timeout_add(600, wait_go)
loop.run()
out["events"] = events
print("RESULT " + json.dumps(out), flush=True)
