#!/bin/bash
cd /home/vjeux/trackmania-tas-tiny/tools
target/release/mapgeom --pak /tmp/BlueBay.pak:660C4C156B80337E296A1034B0AA05B8 --pak /tmp/current-Stadium.pak:B773D73047A4104857722366D78D28A6 crystal-item "$1" --template /tmp/crystal.Item.Gbx --out /tmp/$2.Item.Gbx --ident $2.Item.Gbx --collection 28 2>&1 | grep -v '^warning' | tail -1
