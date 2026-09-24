#!/bin/sh
# Measure the jump across impulse strengths, so the default is chosen from
# data rather than taste.
#
# Prints: strength -> peak height gain, and airtime.
set -u
export TM_SESSION=979f4ff1-2c12-4e17-9d0b-77778502312f
export TM_SESSION_TITLE="jump button"
J=/home/vjeux/bin/jumprig

echo "strength  gain_m  result"
for s in 4 6 8 10 12 15; do
  $J cmd strength $s >/dev/null 2>&1
  # Land and settle before the next measurement, or the previous arc pollutes it.
  $J waitfor grounded 30 >/dev/null 2>&1
  out=$($J jumptest 2>&1)
  gain=$(echo "$out" | sed -n 's/.*gain \([0-9.]*\) m.*/\1/p' | head -1)
  air=$(echo "$out"  | sed -n 's/.*landed .* after \([0-9.]*\)s.*/\1/p' | head -1)
  verdict=$(echo "$out" | grep -o 'RESULT:.*' || echo 'FAILED')
  printf '%-9s %-7s %s (air %ss)\n' "$s" "${gain:-?}" "$verdict" "${air:-?}"
done
