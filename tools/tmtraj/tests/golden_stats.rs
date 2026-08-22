//! Checks the population statistics against every figure the Python-era
//! `reports/analysis.txt` and `reports/analysis2.txt` quote. (The ad-hoc code
//! that produced those two files was not preserved, so this test re-derives the
//! numbers from the same inputs and pins them.)

use tmtraj::lines::{self, Metric, Sort};
use tmtraj::stats;

fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[test]
fn reproduces_the_published_population_analysis() {
    let runs = lines::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/decoder-goldens/paths")).unwrap();
    let an = lines::analyse(runs, Metric::Projection, 300, None, Sort::Time);
    let s = stats::stats(&an);

    // --- analysis.txt: the pairwise distribution -------------------------
    assert_eq!(s.pairs.len(), 1275);
    assert_eq!(r2(s.mean), 3.07);
    assert_eq!(r2(s.sd), 1.38);
    assert_eq!(r2(s.pairs[0]), 0.40);
    assert_eq!(r2(*s.pairs.last().unwrap()), 8.83);
    assert_eq!((s.largest_gap.0 * 1000.0).round() / 1000.0, 0.481);
    assert_eq!((r2(s.largest_gap.1), r2(s.largest_gap.2)), (7.96, 8.44));

    // 0.5 m histogram, exactly the counts printed in analysis.txt
    let want = [3, 22, 96, 177, 212, 205, 154, 122, 97, 63, 46, 28, 18, 19, 5, 5, 1, 2];
    let mut counts = [0usize; 18];
    for p in &s.pairs {
        counts[((p / 0.5) as usize).min(17)] += 1;
    }
    assert_eq!(counts, want);

    // --- analysis.txt: centrality ---------------------------------------
    assert_eq!(r2(s.cent_mean), 3.07);
    assert_eq!(r2(s.cent_sd), 0.63);
    let wr = s.centrality[an.ref_idx].1;
    assert_eq!(r2(wr), 2.57);
    assert_eq!(r2((wr - s.cent_mean) / s.cent_sd), -0.78);
    let mut order = s.centrality.clone();
    order.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    assert_eq!(
        order.iter().position(|(n, _)| n == an.ref_name()).unwrap() + 1,
        10
    );
    let top: Vec<(&str, f64)> = order[..5].iter().map(|(n, v)| (n.as_str(), r2(*v))).collect();
    assert_eq!(
        top,
        vec![
            ("p00044_19581", 2.28),
            ("p00303_19612", 2.31),
            ("p05004_19738", 2.34),
            ("p00301_19611", 2.36),
            ("08_19560", 2.44)
        ]
    );
    let bot: Vec<(&str, f64)> = order[order.len() - 5..]
        .iter()
        .map(|(n, v)| (n.as_str(), r2(*v)))
        .collect();
    assert_eq!(
        bot,
        vec![
            ("p09992_19812", 4.00),
            ("p00701_19628", 4.23),
            ("p01503_19661", 4.27),
            ("p00004_19556", 4.30),
            ("slow_p10000_19812", 5.24)
        ]
    );
    let mut nb: Vec<(usize, f64)> = (0..an.names.len())
        .filter(|&j| j != an.ref_idx)
        .map(|j| (j, an.d[an.ref_idx][j]))
        .collect();
    nb.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    assert_eq!((an.names[nb[0].0].as_str(), r2(nb[0].1)), ("05_19556", 0.70));

    // --- analysis.txt: lateral spread along the lap ----------------------
    let cps = an.cp_stations();
    let at = |k: &str| s.lateral_sd[cps.iter().find(|(n, _)| *n == k).unwrap().1];
    assert_eq!(r2(at("CP1")), 2.13);
    assert_eq!(r2(at("CP2")), 0.71);
    assert_eq!(r2(at("CP3")), 3.34);
    assert_eq!(r2(at("FINISH")), 4.22);
    let mut sd = s.lateral_sd.clone();
    sd.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(r2(sd[0]), 0.00);
    assert_eq!(r2(sd[sd.len() / 2]), 1.44);
    assert_eq!(r2(*sd.last().unwrap()), 4.25);

    // --- analysis2.txt: the most separated pair --------------------------
    assert_eq!(
        (
            s.most_separated.0.as_str(),
            s.most_separated.1.as_str(),
            r2(s.most_separated.2)
        ),
        ("p00004_19556", "slow_p10000_19812", 8.83)
    );
    let cp3 = cps.iter().find(|(n, _)| *n == "CP3").unwrap().1;
    assert_eq!(cp3, 234);
    let a = &an.profiles[&s.most_separated.0];
    let b = &an.profiles[&s.most_separated.1];
    let diff: Vec<f64> = a.iter().zip(b).map(|(x, y)| x - y).collect();
    let peak = diff
        .iter()
        .cloned()
        .fold(0.0f64, |acc, v| if v.abs() > acc.abs() { v } else { acc });
    assert_eq!(r2(peak), -16.26);
    let before_pair = lines::rms(&diff[..cp3]);
    // whole-field restriction to start..CP3
    let n = an.names.len();
    let mut before: Vec<f64> = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            let d: Vec<f64> = an.profiles[&an.names[i]][..cp3]
                .iter()
                .zip(&an.profiles[&an.names[j]][..cp3])
                .map(|(x, y)| x - y)
                .collect();
            before.push(lines::rms(&d));
        }
    }
    let bmean = before.iter().sum::<f64>() / before.len() as f64;
    let bmax = before.iter().cloned().fold(0.0f64, f64::max);
    assert_eq!(r2(bmean), 2.41);
    assert_eq!(r2(bmax), 6.89);
    println!(
        "\nNOTE: analysis2.txt claims this pair's start..CP3 RMS is 7.70 m, but the same file's\n\
         population max over start..CP3 is 6.89 m -- and this pair IS the population max.\n\
         Recomputed here: {:.2} m. The 7.70 figure in the Python-era report is inconsistent\n\
         with its own next line (the script that produced it was not preserved).",
        before_pair
    );
    assert_eq!(r2(before_pair), 6.89);

    // --- analysis2.txt: sector times -------------------------------------
    let wsec = stats::sectors(&an.runs[an.ref_idx].checkpoints_ms);
    assert_eq!(wsec, vec![7617, 5691, 3008, 3222]);
    let mut per: Vec<Vec<f64>> = vec![Vec::new(); 4];
    for r in &an.runs {
        for (i, v) in stats::sectors(&r.checkpoints_ms).iter().enumerate() {
            per[i].push((v - wsec[i]) as f64);
        }
    }
    let med = |mut v: Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        if v.len() % 2 == 0 {
            (v[v.len() / 2 - 1] + v[v.len() / 2]) / 2.0
        } else {
            v[v.len() / 2]
        }
    };
    let medians: Vec<i64> = per.iter().map(|v| med(v.clone()).round() as i64).collect();
    let worst: Vec<i64> = per
        .iter()
        .map(|v| v.iter().cloned().fold(f64::NEG_INFINITY, f64::max) as i64)
        .collect();
    assert_eq!(medians, vec![10, 59, 20, 19]);
    assert_eq!(worst, vec![39, 171, 158, 62]);

    // --- analysis2.txt: speed at 12 stations -----------------------------
    let idx: Vec<usize> = (0..12).map(|k| k * 299 / 11).collect();
    let s_m: Vec<i64> = idx.iter().map(|&i| an.ref_stations[i].0.round() as i64).collect();
    assert_eq!(s_m, vec![0, 164, 329, 493, 657, 822, 992, 1156, 1321, 1485, 1649, 1820]);
    let wrv: Vec<f64> = idx
        .iter()
        .map(|&i| (an.stations_by_run[an.ref_name()][i].4 * 10.0).round() / 10.0)
        .collect();
    assert_eq!(
        wrv,
        vec![0.8, 304.4, 330.5, 324.4, 358.9, 370.8, 404.1, 426.9, 447.8, 471.0, 446.7, 440.5]
    );
    let fieldv: Vec<f64> = idx
        .iter()
        .map(|&i| {
            let v: Vec<f64> = an
                .names
                .iter()
                .filter(|nm| *nm != an.ref_name())
                .map(|nm| an.stations_by_run[nm][i].4)
                .collect();
            (med(v) * 10.0).round() / 10.0
        })
        .collect();
    assert_eq!(
        fieldv,
        vec![0.8, 304.7, 330.9, 323.7, 358.7, 366.4, 400.9, 424.2, 445.0, 467.7, 444.1, 436.6]
    );

    println!("all 60+ published figures in analysis.txt / analysis2.txt reproduced");
}
