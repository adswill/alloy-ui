#!/bin/bash
# usage: shot.sh <binary> <out.png> [wait-seconds]; Linux, Xvfb, software rendering.
xvfb-run -a -s '-screen 0 1280x800x24' bash -c "
  WEBKIT_DISABLE_COMPOSITING_MODE=1 $1 >/dev/null 2>&1 & pid=\$!
  sleep ${3:-6}
  w=\$(xdotool search --onlyvisible --pid \$pid | tail -1)
  if [ -n \"\$w\" ]; then import -window \$w $2; else import -window root -crop 960x640+0+0 +repage $2; fi
  kill \$pid"
