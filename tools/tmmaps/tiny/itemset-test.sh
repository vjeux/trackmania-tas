#!/bin/bash
# itemset-test.sh MAP.Map.Gbx TAG [CAM x,y,z,dist,h,v]...
# The loose-item test: the map (already pushed to Maps/_shoot/) is opened in
# the editor on the render box under the render lock; the probe plugin lists
# what the game kept (an item the game could not resolve is simply absent, a
# missing-items dialog is logged), and one screenshot per camera spec is
# pulled to /tmp/itemset-shots/<TAG>-<n>.png. Each wsx call stays under the
# bridge's 90 s cap; every launching command ends in `; true`.
set -u
if [ "$1" = release ]; then
  /home/vjeux/bin/wsx sh 'S="$HOME/trackmania-tas/tools/target/release/shootctl"; "$S" lock release --owner itemset; pkill -f "[s]leep 1800"; "$S" lock status; true' | tail -2
  exit 0
fi
MAP=$1; TAG=$2; shift 2
S='$HOME/trackmania-tas/tools/target/release/shootctl'
OP=/mnt/c/Users/vjeux/OpenplanetNext
mkdir -p /tmp/itemset-shots
# the lock holder must stay alive (the lock records its shell pid): a detached
# sleeper takes it; `itemset-test.sh release` frees it
/home/vjeux/bin/wsx sh "S=$S; nohup sh -c \"\$S lock acquire --owner itemset --wait 240 >/dev/null 2>&1; sleep 1800\" >/dev/null 2>&1 & sleep 4; \$S lock status; true" | tail -1
# to the menu: leave whatever the game is in
/home/vjeux/bin/wsx sh "S=$S; seen=0; for k in 1 2 3 4 5 6 7 8; do c=\$(\$S get /ctx); case \"\$c\" in *\\\"ctx\\\":0*\\\"dialog\\\":null*) break;; *FrameDialogSaveAs*) \$S get /dismiss >/dev/null; seen=1;; *FrameAskYesNo*) if [ \$seen = 1 ]; then \$S get /yes >/dev/null; else \$S get /no >/dev/null; fi;; *\\\"dialog\\\":null*) \$S get /back >/dev/null;; *) \$S get /dismiss >/dev/null;; esac; sleep 2; done; echo \"menu: \$c\"; true" | tail -1
# open the map in the editor
/home/vjeux/bin/wsx sh "S=$S; rm -f $OP/probe.txt $OP/probe-out.tsv; printf '%s' 'C:/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/$MAP' > $OP/PluginStorage/GhostShooter/editmap.txt; \$S get /editmap >/dev/null; for i in 1 2 3 4 5 6 7 8 9 10; do sleep 6; c=\$(\$S get /ctx); case \"\$c\" in *\\\"ctx\\\":1*) break;; *FrameAskYesNo*) echo 'DIALOG: missing items (answered yes)'; \$S get /yes >/dev/null; sleep 2; \$S get /yes >/dev/null;; esac; done; echo \"editor: \$c\"; true" | tail -2
n=0
for CAM in "$@"; do
  n=$((n+1))
  /home/vjeux/bin/wsx sh "S=$S; sleep 3; printf '%s,%s' '$CAM' '\$\$$n' > $OP/cam.tmp && mv $OP/cam.tmp $OP/cam.txt; for t in 1 2 3 4 5 6; do rm -f $OP/probe-out.tsv; printf 'p%s%s' \$t \$\$ > $OP/probe.txt; sleep 4; [ -s $OP/probe-out.tsv ] && break; done; echo \"--- kept (\$(\$S get /ctx)):\"; grep -v '^item.*-1000' $OP/probe-out.tsv | grep -v '^block' | grep -v '^kind' | grep -v 'Nadeo' | cut -c1-160; sleep 2; /mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe -ExecutionPolicy Bypass -File C:\\\\Users\\\\vjeux\\\\shotdpi.ps1 C:\\\\Users\\\\vjeux\\\\itemset-$TAG-$n.png >/dev/null 2>&1; ls -la /mnt/c/Users/vjeux/itemset-$TAG-$n.png; true" | tail -40
  /home/vjeux/bin/wsx pull /mnt/c/Users/vjeux/itemset-$TAG-$n.png /tmp/itemset-shots/$TAG-$n.png 2>&1 | grep -v chunk | tail -1
done
