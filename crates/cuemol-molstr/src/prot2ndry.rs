//! Protein secondary-structure assignment (Kabsch & Sander DSSP, Biopolymers
//! 1983, 22, 2577-). Faithful port of the C++ `MolCoord::calcProt2ndry` /
//! `calcProt2ndry2` from `../cuemol2/src/modules/molstr/Prot2ndry.cpp`.
//!
//! The result is stored as residue string properties (the form the molvis
//! renderers read via `residue_prop_for_atom`):
//! - `secondary`  = "helix" | "sheet" | (absent for coil)
//! - `secondary2` = "H"|"G"|"I"|"E" with a trailing "s" (start) / "e" (end)
//!   marker, or "" for coil.

use crate::molecule::{MolCoord, ResidueKey};
use cuemol_core::Vector4;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Maximum N(i)-C(i-1) peptide-bond distance before a chain break is assumed.
const PEPDIST_MAX: f64 = 3.0;

const NOBRIDGE: i32 = 0;
const PARALLEL: i32 = 1;
const ANTIPARALLEL: i32 = 2;

/// One backbone residue with the geometry and bookkeeping the DSSP passes need.
#[cfg_attr(test, derive(Clone))]
struct Backbone {
    key: ResidueKey,
    /// Residue name used to exclude proline donors.
    resn: String,
    /// Sequential chain-segment index for peptide connectivity.
    chind: i32,
    ca: Vector4,
    c: Vector4,
    n: Vector4,
    o: Vector4,
    /// Estimated amide-hydrogen position (ZERO when no preceding residue, in
    /// which case the energy term is negligible and forms no bond — as in C++).
    h: Vector4,
    /// Secondary-structure class: "H"/"G"/"I"/"E"/"B"/"T"/" "/"".
    ss: String,
    /// Helix-segment id (1-based) used to place start/end markers.
    ssid: i32,
    /// n-turn marks for n = 3, 4, 5 (chars ' ', '3'..'5', '>', '<', 'X').
    turn: [u8; 3],
    /// Acceptor H-bonds (this residue's N-H donates), sorted by energy ascending.
    /// (C++ also tracks a symmetric donor list, but only acceptors are read.)
    acceptors: Vec<HydrogenBond>,
}

#[derive(Clone, Copy)]
struct HydrogenBond {
    peer: usize,
    energy: f64,
}

#[derive(Clone, Copy)]
struct Bridge {
    btyp: i32,
    ib: i32,
    ie: i32,
    jb: i32,
    je: i32,
    is_isolated: bool,
}

/// Public entry: assign secondary structure (no helix gap-filling).
/// Mirrors C++ `MolCoord::calcProt2ndry(hb_high, bIgnoreBulge)`.
pub fn calc_prot_2ndry(mol: &mut MolCoord, hb_high: f64, ignore_bulge: bool) {
    let mut chains = build_backbone(mol);
    doit(&mut chains, hb_high, ignore_bulge);
    apply_to_mol(mol, &chains);
}

/// Public entry: assign secondary structure, then merge helices separated by a
/// short low-curvature gap. Mirrors C++ `MolCoord::calcProt2ndry2(bIgnoreBulge,
/// dhangl1)`; `hb_high` is fixed at -500 as in the original.
pub fn calc_prot_2ndry2(mol: &mut MolCoord, ignore_bulge: bool, dhangl: f64) {
    let mut chains = build_backbone(mol);
    doit(&mut chains, -500.0, ignore_bulge);
    if dhangl > 0.01 {
        con_helices(&mut chains, dhangl);
    }
    apply_to_mol(mol, &chains);
}

fn doit(chains: &mut [Backbone], hb_high: f64, ignore_bulge: bool) {
    calc_hbon(chains, hb_high);
    calc_bridge(chains, ignore_bulge);
    calc_helices(chains);
}

struct PrevRes {
    chain: String,
    c: Vector4,
    o: Vector4,
}

/// Build the backbone array (C++ `Prot2ndry::init`). Residues missing any of
/// N/CA/C/O break the current segment. The amide H of residue i is estimated
/// from residue i-1's C=O direction.
fn build_backbone(mol: &MolCoord) -> Vec<Backbone> {
    let atoms = mol.atoms();
    let mut chains: Vec<Backbone> = Vec::new();
    let mut prev: Option<PrevRes> = None;
    let mut ci: i32 = 0;

    let mut residues = BTreeSet::new();
    let mut nitrogen = HashMap::new();
    for atom in atoms {
        let key = (atom.chain_id.as_str(), atom.res_id, atom.ins_code);
        residues.insert(key);
        if atom.name.trim() == "N" {
            nitrogen.entry(key).or_insert((atom, [None; 3]));
        }
    }
    for atom in atoms {
        let index = match atom.name.trim() {
            "O" => 0,
            "C" => 1,
            "CA" => 2,
            _ => continue,
        };
        if let Some((_, positions)) =
            nitrogen.get_mut(&(atom.chain_id.as_str(), atom.res_id, atom.ins_code))
        {
            positions[index].get_or_insert(atom.position);
        }
    }
    for (chain, res_id, ins_code) in residues {
        let Some((nitrogen, [Some(o), Some(c), Some(ca)])) =
            nitrogen.get(&(chain, res_id, ins_code))
        else {
            prev = None;
            continue;
        };
        let (n, o, c, ca) = (nitrogen.position, *o, *c, *ca);
        let chain = chain.to_string();
        let mut bb = Backbone {
            key: ResidueKey::new(chain.clone(), res_id, ins_code),
            resn: nitrogen.res_name.clone(),
            chind: 0,
            ca,
            c,
            n,
            o,
            h: Vector4::ZERO,
            ss: String::new(),
            ssid: 0,
            turn: [b' '; 3],
            acceptors: Vec::new(),
        };

        match &prev {
            None => {
                // Start residue of a segment.
                if !chains.is_empty() {
                    ci += 1;
                }
                bb.chind = ci;
            }
            Some(p) => {
                let mut brk = false;
                if p.chain != chain {
                    brk = true;
                } else {
                    let pepdist = n.sub(p.c).length();
                    if pepdist > PEPDIST_MAX {
                        brk = true;
                    } else if let Ok(co) = p.o.sub(p.c).normalize() {
                        bb.h = n.sub(co);
                    }
                }
                if brk {
                    ci += 1;
                }
                bb.chind = ci;
            }
        }

        prev = Some(PrevRes { chain, c, o });
        chains.push(bb);
    }

    chains
}

/// True iff residues i..=j exist and share a chain segment (C++ `noChainBrk`).
fn no_chain_brk(chains: &[Backbone], i: i32, j: i32) -> bool {
    if i > j {
        return false;
    }
    if i < 0 || (j as usize) >= chains.len() {
        return false;
    }
    chains[i as usize].chind == chains[j as usize].chind
}

// ---------------------------------------------------------------------------
// Hydrogen bonds
// ---------------------------------------------------------------------------

fn calc_hbon(chains: &mut [Backbone], hb_high: f64) {
    const CADIST: f64 = 9.0;
    let coordinates = chains
        .iter()
        .map(|bb| [bb.ca.x, bb.ca.y, bb.ca.z].map(|v| (v / CADIST).floor() as i64))
        .collect::<Vec<_>>();
    let mut cells: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
    for (i, &cell) in coordinates.iter().enumerate() {
        cells.entry(cell).or_default().push(i);
    }
    let mut candidates = Vec::new();
    for (i, &[x, y, z]) in coordinates.iter().enumerate() {
        candidates.clear();
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(indices) = cells.get(&[x + dx, y + dy, z + dz]) {
                        candidates.extend(indices.iter().copied().filter(|&j| j > i));
                    }
                }
            }
        }
        candidates.sort_unstable();
        for &j in &candidates {
            if chains[i].ca.sub(chains[j].ca).length() >= CADIST {
                continue;
            }
            check_hbon_pair(chains, i, j, hb_high);
            if j != i + 1 {
                check_hbon_pair(chains, j, i, hb_high);
            }
        }
    }
}

/// Register the bond i(N-H) -> j(C=O) if its energy clears the threshold.
fn check_hbon_pair(chains: &mut [Backbone], i: usize, j: usize, hb_high: f64) {
    let energy = calc_hbon_energy(chains, i, j);
    if energy >= hb_high {
        return;
    }
    let acceptors = &mut chains[i].acceptors;
    if let Err(index) = acceptors.binary_search_by(|bond| bond.energy.total_cmp(&energy)) {
        if index < 2 {
            acceptors.insert(index, HydrogenBond { peer: j, energy });
            acceptors.truncate(2);
        }
    }
}

/// DSSP electrostatic H-bond energy (kcal/mol) for donor i, acceptor j.
fn calc_hbon_energy(chains: &[Backbone], i: usize, j: usize) -> f64 {
    const HBLOW: f64 = -9900.0;
    const DIST: f64 = 0.5;
    const Q: f64 = -27888.0;

    let with = &chains[i];
    if with.resn.eq_ignore_ascii_case("PRO") {
        return 0.0;
    }

    let dho = with.h.sub(chains[j].o).length();
    let dhc = with.h.sub(chains[j].c).length();
    let dnc = with.n.sub(chains[j].c).length();
    let dno = with.n.sub(chains[j].o).length();

    let hbe = if dho < DIST || dhc < DIST || dnc < DIST || dno < DIST {
        HBLOW
    } else {
        Q / dho - Q / dhc + Q / dnc - Q / dno + 0.5
    };

    if hbe > HBLOW {
        hbe
    } else {
        HBLOW
    }
}

/// True iff residue i donates (N-H) to residue j, among i's two strongest bonds.
fn is_hbon(chains: &[Backbone], i: i32, j: i32) -> bool {
    if i < 0 || (i as usize) >= chains.len() {
        return false;
    }
    chains[i as usize]
        .acceptors
        .iter()
        .take(2)
        .any(|hb| hb.peer as i32 == j)
}

// ---------------------------------------------------------------------------
// Beta bridges / sheets
// ---------------------------------------------------------------------------

fn calc_bridge(chains: &mut [Backbone], ignore_bulge: bool) {
    let mut bridges: Vec<Bridge> = Vec::new();
    let n = chains.len();
    let mut neighbors = vec![Vec::new(); n];
    for (i, residue) in chains.iter().enumerate() {
        for bond in &residue.acceptors {
            neighbors[i].push(bond.peer);
            neighbors[bond.peer].push(i);
        }
    }
    let mut candidates = Vec::new();
    for i in 0..n {
        candidates.clear();
        for links in &neighbors[i.saturating_sub(1)..=(i + 1).min(n - 1)] {
            for &peer in links {
                for j in peer.saturating_sub(1)..=(peer + 1).min(n - 1) {
                    if j > i + 2 {
                        candidates.push(j as i32);
                    }
                }
            }
        }
        candidates.sort_unstable();
        candidates.dedup();
        check_bridge(chains, &mut bridges, i as i32, candidates.iter().copied());
    }
    link_strands(chains, &mut bridges, ignore_bulge);
}

fn check_bridge(
    chains: &[Backbone],
    bridges: &mut Vec<Bridge>,
    i: i32,
    candidates: impl Iterator<Item = i32>,
) {
    let mut j1 = 0i32;
    let mut j2 = 0i32;

    if !no_chain_brk(chains, i - 1, i + 1) {
        return;
    }

    for j in candidates {
        if j2 != 0 {
            break;
        }
        if no_chain_brk(chains, j - 1, j + 1) {
            let b = if (is_hbon(chains, i + 1, j) && is_hbon(chains, j, i - 1))
                || (is_hbon(chains, j + 1, i) && is_hbon(chains, i, j - 1))
            {
                PARALLEL
            } else if (is_hbon(chains, i + 1, j - 1) && is_hbon(chains, j + 1, i - 1))
                || (is_hbon(chains, j, i) && is_hbon(chains, i, j))
            {
                ANTIPARALLEL
            } else {
                NOBRIDGE
            };

            if b != NOBRIDGE {
                if j1 == 0 {
                    j1 = j;
                    build_bridge_list(chains, bridges, i, j, b);
                } else if j != j1 {
                    j2 = j;
                    build_bridge_list(chains, bridges, i, j, b);
                }
            }
        }
    }
}

fn build_bridge_list(chains: &[Backbone], bridges: &mut Vec<Bridge>, i: i32, j: i32, btyp: i32) {
    if btyp == NOBRIDGE || i >= j {
        return;
    }

    for br in bridges.iter_mut() {
        if br.btyp != btyp {
            continue;
        }
        if i != br.ie + 1 || !no_chain_brk(chains, br.ie, i) {
            continue;
        }
        if btyp == PARALLEL && j == br.je + 1 && no_chain_brk(chains, br.je, j) {
            br.ie += 1;
            br.je += 1;
            return;
        } else if j == br.jb - 1 && no_chain_brk(chains, j, br.jb) {
            br.ie += 1;
            br.jb -= 1;
            return;
        }
    }

    bridges.push(Bridge {
        btyp,
        ib: i,
        ie: i,
        jb: j,
        je: j,
        is_isolated: true,
    });
}

/// Extend bridge `ai` (and possibly `bi`) when two same-type bridges are
/// separated by a bulge. Mirrors C++ `linkBulge`.
fn link_bulge(bridges: &mut [Bridge], ai: usize, bi: usize) {
    let a = bridges[ai];
    if a.btyp == PARALLEL {
        bridges[bi].ib = a.ie + 1;
        bridges[bi].jb = a.je + 1;
    } else {
        bridges[bi].ib = a.ie + 1;
        bridges[ai].jb = bridges[bi].je + 1;
    }
}

/// Detect (and link) a bulge between two bridges. Mirrors C++ `checkBulge`.
fn check_bulge(bridges: &mut [Bridge], ai: usize, bi: usize) -> bool {
    let a = bridges[ai];
    let b = bridges[bi];
    if a.btyp != b.btyp {
        return false;
    }

    let fits = |idiff: i32, jdiff: i32| {
        1 <= idiff && 1 <= jdiff && ((idiff <= 2 && jdiff <= 5) || (idiff <= 5 && jdiff <= 2))
    };

    if a.btyp == PARALLEL {
        if fits(b.ib - a.ie, b.jb - a.je) {
            link_bulge(bridges, ai, bi);
            return true;
        }
        if fits(a.ib - b.ie, a.jb - b.je) {
            link_bulge(bridges, bi, ai);
            return true;
        }
    } else {
        if fits(b.ib - a.ie, a.jb - b.je) {
            link_bulge(bridges, ai, bi);
            return true;
        }
        if fits(a.ib - b.ie, b.jb - a.je) {
            link_bulge(bridges, bi, ai);
            return true;
        }
    }

    false
}

fn link_strands(chains: &mut [Backbone], bridges: &mut [Bridge], ignore_bulge: bool) {
    let m = bridges.len();
    for i in 0..m {
        if bridges[i].ib != bridges[i].ie {
            bridges[i].is_isolated = false;
        }
        if !ignore_bulge {
            for j in (i + 1)..m {
                if !check_bulge(bridges, i, j) {
                    continue;
                }
                bridges[i].is_isolated = false;
                bridges[j].is_isolated = false;
            }
        }
    }

    for &br in bridges.iter() {
        let cc = if br.is_isolated { "B" } else { "E" };
        mark_strand(chains, br.ib, br.ie, cc);
        mark_strand(chains, br.jb, br.je, cc);
    }
}

/// Mark residues k0..=k1 with `cc`, but never downgrade an existing "E".
fn mark_strand(chains: &mut [Backbone], k0: i32, k1: i32, cc: &str) {
    let mut k = k0;
    while k <= k1 {
        if k >= 0 && (k as usize) < chains.len() && chains[k as usize].ss != "E" {
            chains[k as usize].ss = cc.to_string();
        }
        k += 1;
    }
}

// ---------------------------------------------------------------------------
// Helices and turns
// ---------------------------------------------------------------------------

fn calc_helices(chains: &mut [Backbone]) {
    let n = chains.len() as i32;

    // Mark 3-, 4-, 5-turn structures.
    for k in 3..=5i32 {
        let cc = b'0' + k as u8;
        let idx = (k - 3) as usize;
        let imax = (n - k).max(0);
        for i in 0..imax {
            if !no_chain_brk(chains, i, i + k) {
                continue;
            }
            if !is_hbon(chains, i + k, i) {
                continue;
            }
            chains[(i + k) as usize].turn[idx] = b'<';
            for j in (i + 1)..(i + k) {
                if chains[j as usize].turn[idx] == b' ' {
                    chains[j as usize].turn[idx] = cc;
                }
            }
            chains[i as usize].turn[idx] = if chains[i as usize].turn[idx] == b'<' {
                b'X'
            } else {
                b'>'
            };
        }
    }

    // Shared helix-id counter across the alpha/3-10/pi passes.
    let mut helix_id = 1i32;

    // Alpha helices (4-turn).
    let imax = (n - 4).max(1);
    for i in 1..imax {
        if !is_hx_start(chains, i, 4) {
            helix_id += 1;
            continue;
        }
        for j in i..=(i + 3) {
            chains[j as usize].ss = "H".to_string();
            chains[j as usize].ssid = helix_id;
        }
    }

    // 3-10 helices (3-turn).
    let imax = (n - 3).max(1);
    for i in 1..imax {
        if !is_hx_start(chains, i, 3) {
            helix_id += 1;
            continue;
        }
        if run_is_class(chains, i, i + 2, "G") {
            for j in i..=(i + 2) {
                chains[j as usize].ss = "G".to_string();
                chains[j as usize].ssid = helix_id;
            }
        }
    }

    // Pi helices (5-turn).
    let imax = (n - 5).max(1);
    for i in 1..imax {
        if !is_hx_start(chains, i, 5) {
            helix_id += 1;
            continue;
        }
        if run_is_class(chains, i, i + 4, "I") {
            for j in i..=(i + 4) {
                chains[j as usize].ss = "I".to_string();
                chains[j as usize].ssid = helix_id;
            }
        }
    }

    // Remaining turn residues.
    for i in 0..n {
        if !chains[i as usize].ss.is_empty() {
            continue;
        }
        let mut cc = b' ';
        for turn in 3..=5i32 {
            let mut j = i - (turn - 1);
            while j < i {
                if j >= 0 {
                    let t = chains[j as usize].turn[(turn - 3) as usize];
                    if t == b'X' || t == b'>' {
                        cc = b'T';
                    }
                }
                j += 1;
            }
        }
        chains[i as usize].ss = (cc as char).to_string();
    }
}

/// True iff residues k0..=k1 are all either empty or already class `cls`.
fn run_is_class(chains: &[Backbone], k0: i32, k1: i32, cls: &str) -> bool {
    for j in k0..=k1 {
        let s = &chains[j as usize].ss;
        if s != cls && !s.is_empty() {
            return false;
        }
    }
    true
}

fn is_hx_start(chains: &[Backbone], i: i32, k: i32) -> bool {
    if i - 1 < 0 || (i as usize) >= chains.len() {
        return false;
    }
    let idx = (k - 3) as usize;
    let prev = chains[(i - 1) as usize].turn[idx];
    if prev != b'>' && prev != b'X' {
        return false;
    }
    let cur = chains[i as usize].turn[idx];
    cur == b'>' || cur == b'X'
}

/// Merge helices separated by a short low-curvature gap (C++ `conHelices`).
fn con_helices(chains: &mut [Backbone], dangl: f64) {
    let n = chains.len() as i32;
    let is_helix = |s: &str| s == "H" || s == "G" || s == "I";

    // Collect (start, end) of each contiguous helix run by ssid.
    let mut helices: Vec<(i32, i32)> = Vec::new();
    let mut cur_start = -1i32;
    let mut i = 0i32;
    while i < n {
        if is_helix(&chains[i as usize].ss) {
            let id = chains[i as usize].ssid;
            let previd = if i > 0 && no_chain_brk(chains, i - 1, i) {
                chains[(i - 1) as usize].ssid
            } else {
                -1
            };
            let nextid = if i < n - 1 && no_chain_brk(chains, i, i + 1) {
                chains[(i + 1) as usize].ssid
            } else {
                -1
            };
            if previd != id {
                cur_start = i;
            }
            if nextid != id {
                helices.push((cur_start, i));
                cur_start = -1;
            }
        }
        i += 1;
    }
    if cur_start >= 0 {
        helices.push((cur_start, i));
    }

    if helices.len() < 2 {
        return;
    }

    for h in 0..(helices.len() - 1) {
        let npen = helices[h].1;
        let nnst = helices[h + 1].0;

        let mut ok = true;
        let mut j = npen - 1;
        while j <= nnst + 1 {
            if j >= 0 && j < n && chains[j as usize].ss == "E" {
                ok = false;
                break;
            }
            if j > 0 && j + 2 < n {
                let p1 = chains[(j - 1) as usize].ca;
                let p2 = chains[j as usize].ca;
                let p3 = chains[(j + 1) as usize].ca;
                let p4 = chains[(j + 2) as usize].ca;
                if let Ok(tor) = Vector4::torsion(p1, p2, p3, p4) {
                    let mut tor_deg = tor.to_degrees();
                    if tor_deg < 0.0 {
                        tor_deg += 360.0;
                    }
                    if tor_deg > dangl {
                        ok = false;
                        break;
                    }
                }
            }
            j += 1;
        }
        if !ok {
            continue;
        }

        let nnen = helices[h + 1].1;
        let ss_pen = chains[npen as usize].ss.clone();
        let ssid_pen = chains[npen as usize].ssid;
        let mut k = npen + 1;
        while k <= nnen {
            // Guard: C++ can index one past the end here (UB on a trailing
            // helix); clamp instead of corrupting state.
            if (k as usize) < chains.len() {
                chains[k as usize].ss = ss_pen.clone();
                chains[k as usize].ssid = ssid_pen;
            }
            k += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Apply results back to the molecule
// ---------------------------------------------------------------------------

fn apply_to_mol(mol: &mut MolCoord, chains: &[Backbone]) {
    let mut assignments = Vec::with_capacity(chains.len());
    let n = chains.len();
    for idx in 0..n {
        let bb = &chains[idx];
        let ss = bb.ss.as_str();

        let (secondary, secondary2): (Option<&str>, String) = if ss == "H" || ss == "G" || ss == "I"
        {
            let previd = if idx > 0 { chains[idx - 1].ssid } else { -1 };
            let nextid = if idx + 1 < n {
                chains[idx + 1].ssid
            } else {
                -1
            };
            let mut s2 = ss.to_string();
            if previd != bb.ssid {
                s2.push('s');
            }
            if nextid != bb.ssid {
                s2.push('e');
            }
            (Some("helix"), s2)
        } else if ss == "E" {
            let ss_prev = if idx > 0 {
                chains[idx - 1].ss.as_str()
            } else {
                ""
            };
            let ss_next = if idx + 1 < n {
                chains[idx + 1].ss.as_str()
            } else {
                ""
            };
            if ss_prev != "E" && ss_next == "E" {
                (Some("sheet"), "Es".to_string())
            } else if ss_prev == "E" && ss_next != "E" {
                (Some("sheet"), "Ee".to_string())
            } else if ss_prev != "E" && ss_next != "E" {
                // Isolated x-E-x collapses to coil.
                (None, String::new())
            } else {
                (Some("sheet"), "E".to_string())
            }
        } else {
            (None, String::new())
        };

        assignments.push((bb.key.clone(), secondary, secondary2));
    }
    mol.replace_secondary_structure(assignments);
}

// ---------------------------------------------------------------------------
// Manual secondary-structure editing (Prot2ndrySet)
// ---------------------------------------------------------------------------

/// Per-class residue membership keyed by chain (residue ids). Port of the C++
/// `Prot2ndrySet` helper used by manual secondary-structure editing
/// (`MolAnlManager::setProt2ndry`). Membership is reconstructed into contiguous
/// runs at apply time, so it tolerates gaps in residue numbering.
#[derive(Default)]
pub struct Prot2ndrySet {
    helix: ChainResSet,    // alpha "H"
    helix310: ChainResSet, // "G"
    helixpi: ChainResSet,  // "I"
    sheet: ChainResSet,    // "E"
}

type ChainResSet = BTreeMap<String, BTreeSet<i32>>;

fn chainset_append(set: &mut ChainResSet, chain: &str, st: i32, en: i32) {
    let entry = set.entry(chain.to_string()).or_default();
    for r in st..=en {
        entry.insert(r);
    }
}

fn chainset_remove(set: &mut ChainResSet, chain: &str, st: i32, en: i32) {
    if let Some(entry) = set.get_mut(chain) {
        for r in st..=en {
            entry.remove(&r);
        }
    }
}

fn chainset_contains(set: &ChainResSet, chain: &str, r: i32) -> bool {
    set.get(chain).is_some_and(|s| s.contains(&r))
}

impl Prot2ndrySet {
    /// Read the current `secondary2` residue properties into membership sets
    /// (C++ `Prot2ndrySet::create`). Classification is by the first marker char.
    pub fn create(mol: &MolCoord) -> Self {
        let mut out = Self::default();
        for summary in mol.residue_summaries() {
            let chain = &summary.key.chain_id;
            let sindex = summary.key.script_index();
            let Ok(Some(sec2)) = mol.get_residue_prop_str(chain, &sindex, "secondary2") else {
                continue;
            };
            let rid = summary.key.res_id;
            match sec2.chars().next() {
                Some('H') => chainset_append(&mut out.helix, chain, rid, rid),
                Some('G') => chainset_append(&mut out.helix310, chain, rid, rid),
                Some('I') => chainset_append(&mut out.helixpi, chain, rid, rid),
                Some('E') => chainset_append(&mut out.sheet, chain, rid, rid),
                _ => {}
            }
        }
        out
    }

    /// Force residues [st, en] of `chain` to a secondary-structure type, clearing
    /// the conflicting classes over [st-1, en+1] (C++ `setProt2ndry` switch). Type
    /// codes: 1=sheet, 2=alpha, 3=3-10, 4=pi, other=coil.
    pub fn set_type(&mut self, sec_type: i32, chain: &str, st: i32, en: i32) {
        match sec_type {
            1 => {
                chainset_append(&mut self.sheet, chain, st, en);
                chainset_remove(&mut self.helix, chain, st - 1, en + 1);
                chainset_remove(&mut self.helix310, chain, st - 1, en + 1);
                chainset_remove(&mut self.helixpi, chain, st - 1, en + 1);
            }
            2 => {
                chainset_append(&mut self.helix, chain, st, en);
                chainset_remove(&mut self.sheet, chain, st - 1, en + 1);
                chainset_remove(&mut self.helix310, chain, st - 1, en + 1);
                chainset_remove(&mut self.helixpi, chain, st - 1, en + 1);
            }
            3 => {
                chainset_append(&mut self.helix310, chain, st, en);
                chainset_remove(&mut self.sheet, chain, st - 1, en + 1);
                chainset_remove(&mut self.helix, chain, st - 1, en + 1);
                chainset_remove(&mut self.helixpi, chain, st - 1, en + 1);
            }
            4 => {
                chainset_append(&mut self.helixpi, chain, st, en);
                chainset_remove(&mut self.sheet, chain, st - 1, en + 1);
                chainset_remove(&mut self.helix310, chain, st - 1, en + 1);
                chainset_remove(&mut self.helix, chain, st - 1, en + 1);
            }
            _ => {
                chainset_remove(&mut self.helix, chain, st - 1, en + 1);
                chainset_remove(&mut self.sheet, chain, st - 1, en + 1);
                chainset_remove(&mut self.helix310, chain, st - 1, en + 1);
                chainset_remove(&mut self.helixpi, chain, st - 1, en + 1);
            }
        }
    }

    /// Class (secondary2 char, secondary value) of a residue, if any.
    fn class_of(&self, chain: &str, rid: i32) -> Option<(char, &'static str)> {
        if chainset_contains(&self.helix, chain, rid) {
            Some(('H', "helix"))
        } else if chainset_contains(&self.helix310, chain, rid) {
            Some(('G', "helix"))
        } else if chainset_contains(&self.helixpi, chain, rid) {
            Some(('I', "helix"))
        } else if chainset_contains(&self.sheet, chain, rid) {
            Some(('E', "sheet"))
        } else {
            None
        }
    }

    /// Rewrite the `secondary`/`secondary2` residue properties from the membership
    /// sets (C++ `Prot2ndrySet::applyTo`). Contiguous same-class runs (by residue
    /// sequence order) get start "s"/end "e" markers — start XOR end, so a
    /// single-residue run is marked "s".
    pub fn apply_to(&self, mol: &mut MolCoord) {
        let residues: Vec<(String, String, i32)> = mol
            .residue_summaries()
            .iter()
            .map(|s| (s.key.chain_id.clone(), s.key.script_index(), s.key.res_id))
            .collect();

        for (chain, sindex, _) in &residues {
            let _ = mol.remove_residue_prop_str(chain, sindex, "secondary");
            let _ = mol.remove_residue_prop_str(chain, sindex, "secondary2");
        }

        let n = residues.len();
        let mut i = 0;
        while i < n {
            let (chain, _, rid) = &residues[i];
            let Some((cls, sec)) = self.class_of(chain, *rid) else {
                i += 1;
                continue;
            };
            // Extend the run while same chain and same class.
            let mut j = i;
            while j + 1 < n {
                let (nchain, _, nrid) = &residues[j + 1];
                if nchain != chain || self.class_of(nchain, *nrid).map(|c| c.0) != Some(cls) {
                    break;
                }
                j += 1;
            }
            for (k, (kchain, ksindex, _)) in residues.iter().enumerate().take(j + 1).skip(i) {
                let mut s2 = cls.to_string();
                if k == i {
                    s2.push('s');
                } else if k == j {
                    s2.push('e');
                }
                let _ = mol.set_residue_prop_str(kchain, ksindex, "secondary", sec);
                let _ = mol.set_residue_prop_str(kchain, ksindex, "secondary2", s2);
            }
            i = j + 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backbone(i: usize) -> Backbone {
        let t = i as f64;
        let position = Vector4::new3(12.0 * (t * 0.17).cos(), 12.0 * (t * 0.17).sin(), 0.1 * t);
        Backbone {
            key: ResidueKey::new("A", i as i32, None),
            resn: "ALA".into(),
            chind: (i / 100) as i32,
            ca: position,
            c: position + Vector4::new3(0.3, 0.1, 0.4),
            n: position + Vector4::new3(-0.6, 0.3, 0.0),
            o: position + Vector4::new3(1.2, 0.0, 0.0),
            h: position + Vector4::new3(0.1, 0.3, 0.0),
            ss: String::new(),
            ssid: 0,
            turn: [b' '; 3],
            acceptors: Vec::new(),
        }
    }

    #[test]
    fn proline_never_donates_for_any_residue_name_case() {
        let mut residues = vec![backbone(0), backbone(1)];
        residues[0].h = residues[1].o;
        assert_eq!(calc_hbon_energy(&residues, 0, 1), -9900.0);
        for name in ["PRO", "Pro", "pro"] {
            residues[0].resn = name.into();
            assert_eq!(calc_hbon_energy(&residues, 0, 1), 0.0);
        }
    }

    #[test]
    fn sparse_hydrogen_bonds_and_bridges_match_all_pairs() {
        let mut sparse = (0..400).map(backbone).collect::<Vec<_>>();
        let mut exhaustive = sparse.clone();
        calc_hbon(&mut sparse, -500.0);
        for i in 0..exhaustive.len() {
            for j in i + 1..exhaustive.len() {
                if (exhaustive[i].ca - exhaustive[j].ca).length() >= 9.0 {
                    continue;
                }
                check_hbon_pair(&mut exhaustive, i, j, -500.0);
                if j != i + 1 {
                    check_hbon_pair(&mut exhaustive, j, i, -500.0);
                }
            }
        }
        for (actual, expected) in sparse.iter().zip(&exhaustive) {
            assert_eq!(
                actual
                    .acceptors
                    .iter()
                    .map(|h| (h.peer, h.energy))
                    .collect::<Vec<_>>(),
                expected
                    .acceptors
                    .iter()
                    .map(|h| (h.peer, h.energy))
                    .collect::<Vec<_>>()
            );
            assert!(actual.acceptors.len() <= 2);
        }
        calc_bridge(&mut sparse, false);
        let mut bridges = Vec::new();
        for i in 0..exhaustive.len() as i32 {
            check_bridge(&exhaustive, &mut bridges, i, i + 3..exhaustive.len() as i32);
        }
        link_strands(&mut exhaustive, &mut bridges, false);
        assert_eq!(
            sparse.iter().map(|r| &r.ss).collect::<Vec<_>>(),
            exhaustive.iter().map(|r| &r.ss).collect::<Vec<_>>()
        );
    }
}
