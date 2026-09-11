#!/bin/sh
# box-prune-24h.sh — the box's staging hygiene, one run (cron: hourly). Removes
# staging older than 24 h that every owner can re-create: tinyshots/<dir> (the
# trees / startcheck / lineup PNG sets, banked on the store by their threads),
# ~/shoot/_stage entries (push caches — maps, ghosts, views), Maps\_shoot copies
# (re-staged per shoot). NEVER touches: the Maps folder itself (only its _shoot
# junction target), Maps\Tiny\videos (vjeux's watch folder), ghost inputs
# (tinyvid/ship, the store), tinyvid/mp4 (the render loop prunes that itself).
# Coordinator's word 2026-09-11 08:15Z. Log: ~/shoot/box-prune-24h.log
LOG=/home/vjeux/shoot/box-prune-24h.log
AGE=${1:-1440}   # minutes
{
  echo "== $(date -u +%FT%TZ) before: $(df -m /mnt/c | awk 'NR==2{print $4}') MB free"
  # tinyshots: whole dirs whose newest file is older than AGE
  for d in /mnt/c/Users/vjeux/tinyshots/*/; do
    [ -d "$d" ] || continue
    newest=$(find "$d" -type f -printf '%T@\n' 2>/dev/null | sort -n | tail -1 | cut -d. -f1)
    [ -n "$newest" ] || newest=$(stat -c %Y "$d")
    age_min=$(( ( $(date +%s) - newest ) / 60 ))
    if [ "$age_min" -gt "$AGE" ]; then
      echo "tinyshots: $(basename "$d") ($(du -sm "$d" | cut -f1) MB, ${age_min} min old) removed"
      rm -rf "$d"
    fi
  done
  # _stage: files older than AGE (every one is a push cache)
  n=$(find /home/vjeux/shoot/_stage -maxdepth 1 -type f -mmin +$AGE | wc -l)
  mb=$(find /home/vjeux/shoot/_stage -maxdepth 1 -type f -mmin +$AGE -printf '%s\n' | awk '{s+=$1} END{print int(s/1e6)}')
  find /home/vjeux/shoot/_stage -maxdepth 1 -type f -mmin +$AGE -delete
  echo "_stage: $n file(s), $mb MB removed"
  # Maps\_shoot (the junction target C:\tm\_shoot): staged copies older than AGE
  if [ -d /mnt/c/tm/_shoot ]; then
    n=$(find /mnt/c/tm/_shoot -maxdepth 1 -type f -mmin +$AGE | wc -l)
    find /mnt/c/tm/_shoot -maxdepth 1 -type f -mmin +$AGE -delete
    echo "_shoot: $n staged copy(ies) removed"
  fi
  # the wsx transfer fragments that a killed push leaves behind
  find /home/vjeux/shoot -maxdepth 2 -name '*.wsxpart*' -mmin +60 -delete 2>/dev/null
  echo "== $(date -u +%FT%TZ) after: $(df -m /mnt/c | awk 'NR==2{print $4}') MB free"
} >> "$LOG" 2>&1
tail -n 3 "$LOG"
