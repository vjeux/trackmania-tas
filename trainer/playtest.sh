#!/bin/sh
# End-to-end playtest in a REAL browser: real canvas, real KeyboardEvents, real
# DOM — only the frame clock is ours, so the run reproduces. One line per
# simulated player.
#
#   sh playtest.sh [path-to-chrome]
#
# headless.js is faster and covers the judging logic; this is the check that the
# actual page a person opens behaves the same way.
DIR=$(cd "$(dirname "$0")" && pwd)
CH=${1:-"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"}
OUT=$(mktemp -d)
node -e '
const fs=require("fs"), d=process.argv[1], o=process.argv[2];
let h=fs.readFileSync(d+"/index.html","utf8");
h=h.replace("<script>","<script>"+fs.readFileSync(d+"/playtest-pump.js","utf8")+"</"+"script><script>");
h=h.replace("</body>","<script>"+fs.readFileSync(d+"/playtest-drive.js","utf8")+"</"+"script></body>");
fs.writeFileSync(o+"/pt.html",h);' "$DIR" "$OUT" || exit 1

# Chrome on this box does not always exit after --dump-dom, so: run it detached,
# wait for the verdict to land in the file, then kill it. Never pipe its stdout
# — closing the pipe early wedges it.
"$CH" --headless=new --disable-gpu --no-first-run --user-data-dir="$OUT/profile" \
      --virtual-time-budget=20000 --dump-dom "file://$OUT/pt.html" >"$OUT/dom.html" 2>/dev/null &
PID=$!
i=0
while [ $i -lt 60 ]; do
  grep -q '<title>RESULT\|<title>ERR' "$OUT/dom.html" 2>/dev/null && break
  i=$((i+1)); sleep 1
done
kill $PID 2>/dev/null; wait $PID 2>/dev/null

if grep -q '<title>' "$OUT/dom.html" 2>/dev/null; then
  grep -o '<title>[^<]*</title>' "$OUT/dom.html" | head -1 \
    | sed -e 's|<title>||' -e 's|</title>||' -e 's/ | /@/g' | tr '@' '\n'
  RC=0
else
  echo "playtest: no verdict — chrome produced no DOM (is the path right?)"; RC=1
fi
rm -rf "$OUT"
exit $RC
