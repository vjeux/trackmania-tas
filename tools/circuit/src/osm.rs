//! The circuit's outline from an OpenStreetMap Overpass dump (`out body` of
//! the raceway ways with their nodes). OSM draws a circuit as centreline
//! ways broken at every junction and corner name; the Grand Prix lap is the
//! shortest closed route through the named corners in their known order,
//! never through anything named as another layout (pit lanes, the Stowe
//! circuit, the pre-2010 Bridge/Priory section).

use crate::geo::{wgs84_to_bng, Bng};
use serde::Deserialize;
use std::collections::{BTreeMap, BinaryHeap, HashMap};
use std::path::Path;

#[derive(Deserialize)]
struct Dump {
    elements: Vec<Element>,
}

#[derive(Deserialize)]
struct Element {
    #[serde(rename = "type")]
    kind: String,
    id: i64,
    #[serde(default)]
    lat: f64,
    #[serde(default)]
    lon: f64,
    #[serde(default)]
    nodes: Vec<i64>,
    #[serde(default)]
    tags: BTreeMap<String, String>,
}

pub struct Way {
    pub id: i64,
    pub name: String,
    pub tags: BTreeMap<String, String>,
    /// Node ids in way order.
    pub nodes: Vec<i64>,
}

pub struct Ways {
    pub nodes: HashMap<i64, Bng>,
    pub ways: Vec<Way>,
    /// Tagged nodes (`raceway=start`, `raceway=finish`, ...): the tags by id.
    pub node_tags: HashMap<i64, BTreeMap<String, String>>,
}

impl Ways {
    /// The node carrying `key=value`, if any.
    pub fn tagged_node(&self, key: &str, value: &str) -> Option<Bng> {
        self.node_tags.iter().find(|(_, t)| t.get(key).map(|v| v == value).unwrap_or(false)).and_then(|(id, _)| self.nodes.get(id).copied())
    }
}

pub fn load(path: &Path) -> Ways {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let d: Dump = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut nodes = HashMap::new();
    let mut ways = Vec::new();
    let mut node_tags = HashMap::new();
    for el in d.elements {
        match el.kind.as_str() {
            "node" => {
                nodes.insert(el.id, wgs84_to_bng(el.lat, el.lon));
                if !el.tags.is_empty() {
                    node_tags.insert(el.id, el.tags);
                }
            }
            "way" => {
                let name = el.tags.get("name").cloned().unwrap_or_default();
                ways.push(Way { id: el.id, name, tags: el.tags, nodes: el.nodes });
            }
            _ => {}
        }
    }
    Ways { nodes, ways, node_tags }
}

/// The Grand Prix corners in lap order, starting on the pit straight.
pub const GP_ORDER: &[&str] = &[
    "Hamilton Straight",
    "Abbey",
    "Farm Curve",
    "Village",
    "The Loop",
    "Aintree",
    "Wellington Straight",
    "Brooklands",
    "Luffield",
    "Woodcote",
    "Copse",
    "Maggotts",
    "Becketts",
    "Chapel Curve",
    "Hangar Straight",
    "Stowe",
    "Vale",
    "Club",
];

/// Names that belong to other layouts and must never carry the GP lap.
const FORBIDDEN: &[&str] = &["Stowe Circuit", "Stowe Circuit Pit", "Stowe CircuitPit", "National pit lane", "International pit lane", "Bridge", "Priory", "Ice Hill", "Kick Plate", "Limestone Curves"];

pub struct Loop {
    /// Closed polyline (last point != first; the loop closes implicitly).
    pub points: Vec<Bng>,
    /// The corner name the point was reached under.
    pub labels: Vec<String>,
    pub way_names: Vec<String>,
}

impl Loop {
    pub fn length(&self) -> f64 {
        let n = self.points.len();
        (0..n).map(|i| dist(self.points[i], self.points[(i + 1) % n])).sum()
    }
}

fn dist(a: Bng, b: Bng) -> f64 {
    ((a.e - b.e).powi(2) + (a.n - b.n).powi(2)).sqrt()
}

fn usable(w: &Way) -> bool {
    if w.tags.get("highway").map(|s| s.as_str()) != Some("raceway") {
        return false;
    }
    if FORBIDDEN.contains(&w.name.as_str()) {
        return false;
    }
    if w.tags.get("service").is_some() {
        return false;
    }
    if matches!(w.tags.get("surface").map(|s| s.as_str()), Some("unpaved") | Some("dirt") | Some("gravel") | Some("grass")) {
        return false;
    }
    true
}

/// Shortest path over the usable raceway graph between two nodes.
fn shortest(adj: &HashMap<i64, Vec<(i64, f64)>>, from: i64, to: i64) -> Option<Vec<i64>> {
    #[derive(PartialEq)]
    struct Q(f64, i64);
    impl Eq for Q {}
    impl PartialOrd for Q {
        fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(o))
        }
    }
    impl Ord for Q {
        fn cmp(&self, o: &Self) -> std::cmp::Ordering {
            o.0.partial_cmp(&self.0).unwrap_or(std::cmp::Ordering::Equal)
        }
    }
    let mut best: HashMap<i64, f64> = HashMap::new();
    let mut prev: HashMap<i64, i64> = HashMap::new();
    let mut heap = BinaryHeap::new();
    best.insert(from, 0.0);
    heap.push(Q(0.0, from));
    while let Some(Q(d, u)) = heap.pop() {
        if u == to {
            let mut path = vec![to];
            let mut c = to;
            while let Some(&p) = prev.get(&c) {
                path.push(p);
                c = p;
            }
            path.reverse();
            return Some(path);
        }
        if d > *best.get(&u).unwrap_or(&f64::MAX) {
            continue;
        }
        for &(v, w) in adj.get(&u).map(|v| v.as_slice()).unwrap_or(&[]) {
            let nd = d + w;
            if nd < *best.get(&v).unwrap_or(&f64::MAX) {
                best.insert(v, nd);
                prev.insert(v, u);
                heap.push(Q(nd, v));
            }
        }
    }
    None
}

/// The GP lap through the named corners. Each corner name may be split over
/// several ways; the lap visits them all, nearest first, before moving on.
pub fn gp_loop(w: &Ways) -> Loop {
    let mut adj: HashMap<i64, Vec<(i64, f64)>> = HashMap::new();
    for way in w.ways.iter().filter(|w| usable(w)) {
        for k in 1..way.nodes.len() {
            let (a, b) = (way.nodes[k - 1], way.nodes[k]);
            let d = dist(w.nodes[&a], w.nodes[&b]);
            adj.entry(a).or_default().push((b, d));
            adj.entry(b).or_default().push((a, d));
        }
    }
    // A checkpoint per named way: its middle node.
    let mut checkpoints: Vec<(String, Vec<i64>)> = Vec::new();
    for name in GP_ORDER {
        let mids: Vec<i64> = w.ways.iter().filter(|x| usable(x) && x.name == *name).map(|x| x.nodes[x.nodes.len() / 2]).collect();
        assert!(!mids.is_empty(), "no usable raceway way named {name:?} in the dump");
        checkpoints.push((name.to_string(), mids));
    }
    let start = checkpoints[0].1[0];
    let mut order: Vec<(String, i64)> = vec![(checkpoints[0].0.clone(), start)];
    let mut cur = start;
    for (name, mids) in checkpoints.iter().skip(1) {
        let mut left: Vec<i64> = mids.clone();
        while !left.is_empty() {
            let (k, _) = left
                .iter()
                .enumerate()
                .map(|(k, &m)| (k, shortest(&adj, cur, m).map(|p| p.len()).unwrap_or(usize::MAX)))
                .min_by_key(|(_, l)| *l)
                .unwrap();
            cur = left.remove(k);
            order.push((name.clone(), cur));
        }
    }
    let mut points: Vec<Bng> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    let mut way_names: Vec<String> = Vec::new();
    for i in 0..order.len() {
        let (name, a) = &order[i];
        let (_, b) = &order[(i + 1) % order.len()];
        let path = shortest(&adj, *a, *b).unwrap_or_else(|| panic!("no raceway path from {name} onwards"));
        for &nid in &path[..path.len() - 1] {
            points.push(w.nodes[&nid]);
            labels.push(name.clone());
        }
        way_names.push(name.clone());
    }
    Loop { points, labels, way_names }
}
