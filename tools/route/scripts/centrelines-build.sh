#!/bin/bash
# ship7/ship8 centrelines: AUTHOR order (from the source ghost, when complete) as the main route, the planner's order as
# rank 1; engine spawns, verdicts, exclusions, hand drive lines, author-line oracle; one drop per build.
source /tmp/tmp/env.sh; cd /tmp/tmp/repo/tools/route
M=/tmp/tmp/bank-mirror; REAL=$HOME/persistent/private-30d/tm-route
BUILD=${1:?build}; B7=$M/tm-route/tiny-builds/$BUILD; T7=$M/tm-player/tiny/$BUILD
A=$HOME/persistent/private-30d/tm-player/tiny/incoming/$BUILD/ANCHORS.tsv; [ -f $A ] || A=$HOME/persistent/private-30d/tm-player/tiny/incoming/ship8-ca2c04fd/ANCHORS.tsv
V=$M/tm-route/tiny/gap-verdicts-$BUILD.tsv; [ -f $V ] || V=$M/tm-route/tiny/gap-verdicts-ship7-3adf1f85.tsv; X=$M/tm-route/tiny/road-exclusions.tsv; LL=$M/tm-route/tiny/leg-lines.tsv
: > $B7/centreline/ORDERS.tsv
for f in $B7/centreline/*.road-centreline.json; do
  b=$(basename $f .road-centreline.json); nn=${b:0:2}
  plan_order=$(grep -o '"order_groups": \[[^]]*\]' $f | tr -d '"order_groups: []')
  [ -f $B7/centreline/$b.planner-order.txt ] && plan_order=$(cat $B7/centreline/$b.planner-order.txt)
  echo "$plan_order" > $B7/centreline/$b.planner-order.txt
  n_groups=$(echo "$plan_order" | tr ',' '\n' | wc -l)
  note=$(grep -o '"produced_by": "[^"]*' $f | sed 's/"produced_by": "//; s/^tmplan road-centreline[^;]*; //; s/; verdicts 08:50Z applied.*$//; s/; spawn = engine.*$//; s/; order = AUTHOR.*$//')
  sp=$(awk -F'\t' -v n="$nn" '$1==n {print $2","$3","$4}' $M/tm-player/tiny/gate-crossings/ENGINE-SPAWNS-*.tsv 2>/dev/null | head -1); SPAWN=""; [ -n "$sp" ] && SPAWN="--spawn $sp"
  # the author's order from the source ghost
  author_order=""; row=$(awk -F'\t' -v n=$nn '$1==n' $A 2>/dev/null); GH=""
  if [ -n "$row" ]; then
    src=/tmp/summer2026/$(basename "$(echo "$row" | cut -f2)"); anc=$(echo "$row" | awk -F'\t' '{print $3","$4","$5":"$6","$7","$8}')
    ghost_bytes=$(echo "$row" | cut -f10); if [ -n "$ghost_bytes" ] && [ "$ghost_bytes" -gt 12 ] 2>/dev/null || { [ -z "$ghost_bytes" ] && target/release/tmplan author-line "$src" --anchor $anc --centreline $f 2>/dev/null | grep -q "author line vs"; }; then GH="$src"; else
      # no author ghost in the source: the bank's best full-size human ghost of the same map (tm-player crawl, rank 1)
      fu=$(target/release/tmroute gates "$src" 2>/dev/null | head -1 | cut -f2); tarf=$HOME/persistent/private-30d/tm-player/data/v0/maps/$fu/ghosts.tar
      if [ -n "$fu" ] && [ -f "$tarf" ]; then mkdir -p /tmp/ghosts-$fu; [ -n "$(ls /tmp/ghosts-$fu/ghosts 2>/dev/null)" ] || tar xf "$tarf" -C /tmp/ghosts-$fu; GH=$(ls /tmp/ghosts-$fu/ghosts/1-*.Ghost.Gbx 2>/dev/null | head -1); fi
    fi
  fi
  if [ -n "$GH" ]; then
    src="$GH"
    author_order=$(target/release/tmplan author-line "$src" --anchor $anc --centreline $f --gates $B7/geom/$b.deck.json --out $B7/centreline/$b.author-line.json 2>/dev/null | grep "author order" | sed 's/.*: //')
    [ "$(echo "$author_order" | tr ',' '\n' | grep -c .)" -ge "$n_groups" ] || { echo "$b: author order incomplete ($author_order) — planner order kept" >&2; author_order=""; }
  fi
  main_order=${author_order:-$plan_order}; src_note="order = planner (hybrid)"; AL=""; [ -n "$author_order" ] && { src_note="order = AUTHOR (source ghost first-pass); planner order kept as rank 1"; AL="--author-line $B7/centreline/$b.author-line.json"; }
  echo -e "$b\t$plan_order\t${author_order:--}\t$([ "$author_order" = "$plan_order" ] && echo same || ([ -z "$author_order" ] && echo n/a || echo DIFFER))" >> $B7/centreline/ORDERS.tsv
  nice target/release/tmplan road-centreline "$T7/by-stem/$b.Map.Gbx" --gates $B7/geom/$b.deck.json --order $main_order --out $B7/centreline/$b.road-centreline${RV:+.$RV}.json --route-out $B7/centreline/$b.route-router-road-centreline${RV:+-$RV}-0.json --verdicts $V --exclusions $X --leg-lines $LL --map-stem $b $SPAWN $AL --note "$note; $src_note" > $B7/centreline/$b.centreline.txt 2>&1
  if [ -n "$author_order" ] && [ "$author_order" != "$plan_order" ]; then
    nice target/release/tmplan road-centreline "$T7/by-stem/$b.Map.Gbx" --gates $B7/geom/$b.deck.json --order $plan_order --out $B7/centreline/$b.road-centreline.planner-order${RV:+.$RV}.json --route-out $B7/centreline/$b.route-router-road-centreline${RV:+-$RV}-1.json --verdicts $V --exclusions $X --leg-lines $LL --map-stem $b $SPAWN --note "$note; order = planner (hybrid), rank 1" > /dev/null 2>&1
  else rm -f $B7/centreline/$b.road-centreline.planner-order${RV:+.$RV}.json $B7/centreline/$b.route-router-road-centreline${RV:+-$RV}-1.json; fi
  echo "$b [$([ -n "$author_order" ] && echo author || echo planner) order $main_order] $(grep -o 'pts,.*%' $B7/centreline/$b.centreline.txt | head -1) $(grep -o '"connection":"[A-Za-z]*"' $B7/centreline/$b.route-router-road-centreline${RV:+-$RV}-0.json | sort | uniq -c | tr -s ' \n' ' ')"
done
bash /tmp/author-lines-build.sh $BUILD > /dev/null 2>&1

ts=$(date -u +%Y%m%dT%H%MZ)
for B in $BUILD; do tar czf /tmp/tiny-$B-$ts.tgz -C $M/tm-route/tiny-builds $B && cp /tmp/tiny-$B-$ts.tgz $REAL/drops/ && echo -e "$ts\ttiny-$B-$ts.tgz\ttiny build $B: centrelines in the AUTHOR's order where the source has a ghost (planner order as rank 1), engine spawns, stubs, credit-plane gates, hand drive lines, author-line oracle\t$(stat -c %s /tmp/tiny-$B-$ts.tgz)\t$(md5sum < /tmp/tiny-$B-$ts.tgz | cut -c1-12)" >> $REAL/drops/MANIFEST.tsv; rm -rf $REAL/tiny/$B/centreline; tar xzf /tmp/tiny-$B-$ts.tgz -C $REAL/tiny/ $B/centreline; done
echo "done $ts"
