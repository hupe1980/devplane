#!/bin/sh
# The .deb's post-removal script (wired as `bundle.linux.deb.postRemoveScript`
# in tauri.conf.json). It deletes nothing: history and the person's own
# preferences live in ~/.devplane, which is left in place and named.
set -e

case "$1" in
  remove|purge)
    echo "Devplane was removed. Your history in ~/.devplane was left in place."
    ;;
esac

exit 0
