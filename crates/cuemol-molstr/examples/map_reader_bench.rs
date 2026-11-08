use cuemol_molstr::{read_opendx_str, read_xplor_map_str, GridMap};
use std::{collections::hash_map::DefaultHasher, hash::Hasher, time::Instant};

fn hash(map: &GridMap) -> u64 {
    let mut hash = DefaultHasher::new();
    for count in map.counts {
        hash.write_usize(count);
    }
    for vector in std::iter::once(&map.origin).chain(map.delta.iter()) {
        for value in [vector.x(), vector.y(), vector.z()] {
            hash.write_u64(value.to_bits());
        }
    }
    for value in &map.values {
        hash.write_u64(value.to_bits());
    }
    hash.finish()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments.len() != 2 {
        return Err("usage: map_reader_bench INPUT_DX_OR_XPLOR ITERATIONS".into());
    }
    let iterations: usize = arguments[1].parse()?;
    if iterations == 0 {
        return Err("iterations must be positive".into());
    }
    let input = std::fs::read_to_string(&arguments[0])?;
    let reader = if arguments[0].ends_with(".dx") {
        read_opendx_str
    } else {
        read_xplor_map_str
    };
    let mut expected = None;
    for iteration in 0..iterations {
        let start = Instant::now();
        let map = reader(&input)?;
        let read_ms = start.elapsed().as_secs_f64() * 1000.;
        let hash = hash(&map);
        if let Some(expected) = expected {
            assert_eq!(hash, expected, "repeated reads changed the map");
        }
        expected = Some(hash);
        println!(
            "{{\"iteration\":{iteration},\"values\":{},\"counts\":{:?},\"read_ms\":{read_ms},\"hash\":\"{hash:016x}\"}}",
            map.values.len(), map.counts
        );
    }
    Ok(())
}
