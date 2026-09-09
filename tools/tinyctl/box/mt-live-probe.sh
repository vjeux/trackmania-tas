#!/bin/sh
# probe12.sh NN — render setup, hide the MT interface, screenshot, show it again
S=/home/vjeux/trackmania-tas/tools/target/release/shootctl
NN=$1
L=/home/vjeux/shoot/probe12-$NN.log
: > $L
$S lock acquire --owner "vid2-probe12-$NN ~3min" --wait 900 >> $L 2>&1 || { echo "lock timeout" >> $L; exit 1; }
cp "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/Tiny/Tiny Summer 2026 - $NN.Map.Gbx" /home/vjeux/shoot/_stage/vid$NN.Map.Gbx
cp /home/vjeux/shoot/_stage/vid$NN.Map.Gbx "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/vid$NN.Map.Gbx"
$S setup --map "C:/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/vid$NN.Map.Gbx" --cam 2 /home/vjeux/shoot/_stage/vid$NN.Ghost.Gbx >> $L 2>&1
echo "--- hide ui" >> $L; $S get "/mtui?hide=1" >> $L 2>&1
$S get /rewind >> $L 2>&1
sleep 2
powershell.exe -ExecutionPolicy Bypass -File "C:\\Users\\vjeux\\shotdpi.ps1" "C:\\Users\\vjeux\\tinyvid\\probe12-$NN-hidden.png" >> $L 2>&1
$S get /play >> $L 2>&1
sleep 3
powershell.exe -ExecutionPolicy Bypass -File "C:\\Users\\vjeux\\shotdpi.ps1" "C:\\Users\\vjeux\\tinyvid\\probe12-$NN-playing.png" >> $L 2>&1
$S get /stop >> $L 2>&1
echo "--- show ui" >> $L; $S get "/mtui?hide=0" >> $L 2>&1
rm -f "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/vid$NN.Map.Gbx"
$S lock release --owner "vid2-probe12-$NN ~3min" >> $L 2>&1
echo PROBE_DONE >> $L
