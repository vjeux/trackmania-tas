//! The parsers behind `tmsite records`, on fixtures shaped like the real
//! thing. Each case here is one the live refresh actually hit.

use tmsite::records::*;

#[test]
fn caption_the_ordinary_form() {
    let l = "**Tap water 01** — TAS **22.072** (−1.253) | AT 23.325 | WR 23.298 by Lukrecja666";
    let c = caption_wr(l).expect("a caption");
    assert_eq!(c.wr_ms, Some(23_298));
    assert_eq!(c.holder.as_deref(), Some("Lukrecja666"));
    assert_eq!(caption_at(l), Some(23_325));
}

#[test]
fn caption_a_tie_names_no_holder() {
    let c = caption_wr("**Fall 2025 - 18** — TAS **4.492** (±0) | AT 4.492 | WR 4.495 (six players tied)")
        .expect("a caption");
    assert_eq!(c.wr_ms, Some(4_495));
    assert_eq!(c.holder, None);
}

#[test]
fn caption_nobody_is_an_absence_not_a_player() {
    let c = caption_wr("**Spring 2023 - 15 (Underwater)** — TAS **36.049** (not a completion — a landing) | AT 2672.290 | WR — by nobody (0 online records)")
        .expect("a caption");
    assert_eq!(c.wr_ms, None);
    assert_eq!(c.holder, None);
}

#[test]
fn caption_holder_in_backticks() {
    let c = caption_wr("**impossible at for ssano** — TAS **14.289** | AT 14.648 | WR 15.039 by `in-.-`")
        .expect("a caption");
    assert_eq!(c.holder.as_deref(), Some("in-.-"));
}

#[test]
fn a_signed_number_is_a_delta_not_a_time() {
    assert_eq!(page_ms("**−0.646**"), None);
    assert_eq!(page_ms("+7.018"), None);
    assert_eq!(page_ms("**4.492**"), Some(4_492));
    assert_eq!(page_ms("2540.641"), Some(2_540_641));
    assert_eq!(page_ms("—"), None);
    assert_eq!(page_count("1,052"), Some(1052));
    assert_eq!(page_count("**1**"), Some(1));
    assert_eq!(page_count("*none*"), None);
}

#[test]
fn root_table_rows_including_a_bracketed_name() {
    let md = "\
| map | records | author time | best human | **this TAS** | vs AT |\n\
|---|---|---|---|---|---|\n\
| [[Turtle Trial] Angustus](238835-turtle-trial-angustus) | 1 | 462.982 | 1964.933 | **239.133** | **−48.3 %** |\n\
| [Fall 2025 - 18 CP1 End](270053-fall-2025-18-cp1-end) | 1,052 | 4.492 | 4.495 | **4.492** | **±0** |\n\
| [untitled 01](276874-untitled-01) | **0** | 23.839 | *none* | **12.759** | **−46.5 %** |\n";
    let rows = root_rows(md);
    let a = rows.get("238835-turtle-trial-angustus").expect("angustus");
    assert_eq!(a.name, "[Turtle Trial] Angustus");
    assert_eq!(a.records, Some(1));
    assert_eq!(a.at_ms, Some(462_982));
    assert_eq!(a.human_ms, Some(1_964_933));
    assert_eq!(a.tas_ms, Some(239_133));
    let f = rows.get("270053-fall-2025-18-cp1-end").expect("fall 18");
    assert_eq!(f.records, Some(1052));
    let u = rows.get("276874-untitled-01").expect("untitled 01");
    assert_eq!(u.records, Some(0));
    assert_eq!(u.human_ms, None, "*none* is not a time");
}

#[test]
fn a_board_is_read_by_position_and_playercount() {
    let j = r#"{"tops":[
        {"player":{"name":"AffiTM"},"position":1,"time":4495,"timestamp":"2026-04-29T15:04:24+00:00"},
        {"player":{"name":"Verezz"},"position":2,"time":4495,"timestamp":"2026-04-29T15:13:59+00:00"},
        {"player":{"name":"third"},"position":3,"time":4497,"timestamp":"2026-05-01T00:00:00+00:00"}
    ],"playercount":1078,"lockedLeaderboard":false}"#;
    let b = board(j).expect("a board");
    assert_eq!(b.records, Some(1078));
    assert_eq!(b.tops.len(), 3);
    assert_eq!(b.tops[0].player, "AffiTM");
    assert_eq!(b.tops[0].time_ms, 4495);
    assert_eq!(b.tops[0].when, "2026-04-29T15:04:24+00:00");
}

#[test]
fn an_empty_board_parses_as_empty_not_as_an_error() {
    // The shape 276874 returns: no records at all. It must not look like a
    // failure, and it must not look like a record either.
    let b = board(r#"{"tops":null,"playercount":0}"#).expect("still valid JSON");
    assert!(b.tops.is_empty());
    assert_eq!(b.records, Some(0));
}

#[test]
fn tmx_batch_gives_uid_and_its_own_mirror() {
    let j = r#"{"More":false,"Results":[
      {"MapId":270053,"MapUid":"6r7HjKPCuImnLMBfqiKwWpGK1U1","Name":"Fall 2025 - 18 CP1 End",
       "Medals":{"Author":4492},
       "OnlineWR":{"AccountId":"A9","DisplayName":"AffiTM","RecordTime":4495,"User":null},
       "OnlineRecordCount":1078},
      {"MapId":276874,"MapUid":"9wv8HirGqNFCJsFeVJg6ErKYH6b","Name":"untitled 01",
       "Medals":{"Author":23839},
       "OnlineWR":{"AccountId":null,"DisplayName":null,"RecordTime":0,"User":null},
       "OnlineRecordCount":0}]}"#;
    assert_eq!(
        tmx_uids(j),
        vec![
            (270053, "6r7HjKPCuImnLMBfqiKwWpGK1U1".to_string()),
            (276874, "9wv8HirGqNFCJsFeVJg6ErKYH6b".to_string())
        ]
    );
    let rows = tmx_rows(j);
    assert_eq!(rows[0].1.author_ms, Some(4492));
    assert_eq!(rows[0].1.wr_ms, Some(4495));
    assert_eq!(rows[0].1.wr_holder.as_deref(), Some("AffiTM"));
    assert_eq!(rows[0].1.records, Some(1078));
    assert_eq!(rows[1].1.wr_ms, None, "RecordTime 0 with a null holder is no record, not 0.000");
    assert_eq!(rows[1].1.records, Some(0));
}

#[test]
fn map_info_gives_the_author_score() {
    let j = r#"{"name":"$o$i$aa0Kack$05ay","authorScore":24062,"mapUid":"NTU3ZGRlMzEtYzNiOC00YzJmLTk"}"#;
    let (name, at) = map_info(j).expect("map info");
    assert_eq!(at, Some(24_062));
    assert!(name.contains("Kack"));
}
