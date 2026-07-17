pub const CUBE_VERTEX_OFFSET: [[usize; 3]; 8] = [
    [0, 0, 0],
    [1, 0, 0],
    [1, 1, 0],
    [0, 1, 0],
    [0, 0, 1],
    [1, 0, 1],
    [1, 1, 1],
    [0, 1, 1],
];

pub const CUBE_EDGE_CONNECTION: [[usize; 2]; 12] = [
    [0, 1],
    [1, 2],
    [2, 3],
    [3, 0],
    [4, 5],
    [5, 6],
    [6, 7],
    [7, 4],
    [0, 4],
    [1, 5],
    [2, 6],
    [3, 7],
];

const fn make_cube_edge_flags() -> [u16; 256] {
    let mut flags = [0_u16; 256];
    let mut mask = 0;
    while mask < flags.len() {
        let mut edge = 0;
        while edge < CUBE_EDGE_CONNECTION.len() {
            let corners = CUBE_EDGE_CONNECTION[edge];
            let inside0 = mask & (1 << corners[0]) != 0;
            let inside1 = mask & (1 << corners[1]) != 0;
            if inside0 != inside1 {
                flags[mask] |= 1 << edge;
            }
            edge += 1;
        }
        mask += 1;
    }
    flags
}

pub const CUBE_EDGE_FLAGS: [u16; 256] = make_cube_edge_flags();

const TRIANGLE_NIBBLES: &str = "ffffffffffffffff083fffffffffffff019fffffffffffff183981ffffffffff12afffffffffffff08312affffffffff92a029ffffffffff2832a8a98fffffff3b2fffffffffffff0b28b0ffffffffff19023bffffffffff1b219b98bfffffff3a1ba3ffffffffff0a108a8bafffffff3903b9ba9fffffff98aa8bffffffffff478fffffffffffff430734ffffffffff019847ffffffffff419471731fffffff12a847ffffffffff34730412afffffff92a902847fffffff2a9297273794ffff8473b2ffffffffffb47b24204fffffff90184723bfffffff47b94b9b2921ffff3a13ba784fffffff1ba14b1047b4ffff47890b9bab03ffff47b4b99bafffffff954fffffffffffff954083ffffffffff054150ffffffffff854835315fffffff12a954ffffffffff30812a495fffffff52a542402fffffff2a5325354348ffff95423bffffffffff0b208b495fffffff05401523bfffffff21525828b485ffffa3ba13954fffffff4950818a18baffff54050b5bab03ffff54858aa8bfffffff978579ffffffffff930953573fffffff078017157fffffff153357ffffffffff978957a12fffffffa12950530573ffff802825857a52ffff2a5253357fffffff7957893b2fffffff95797292027bffff23b018178157ffffb21b17715fffffff958857a13a3bffff5705097b010aba0fba0b03a50807570fba57b5ffffffffffa65fffffffffffff0835a6ffffffffff9015a6ffffffffff1831985a6fffffff165261ffffffffff165126308fffffff965906026fffffff598582526328ffff23ba65ffffffffffb08b20a65fffffff01923b5a6fffffff5a61929b298bffff63b653513fffffff08b0b50515b6ffff3b6036065059ffff65969bb98fffffff5a6478ffffffffff43047365afffffff1905a6847fffffffa65197173794ffff612651478fffffff125526304347ffff847905065026ffff739794329596269f3b2784a65fffffff5a647242027bffff01947823b5a6ffff9219b294b7b45a6f8473b53515b6ffff51b5b610b7b404bf059065036b63847f65969b4797b9ffffa4964affffffffff4a649a083fffffffa01a60640fffffff83181686461affff149124264fffffff308129249264ffff024426ffffffffff832824426fffffffa49a64b23fffffff08228b49a4a6ffff3b201606461affff64161a48121b8b1f964936913b63ffff8b1810b61914641f3b6360064fffffff648b68ffffffffff7a678a89afffffff0730a709a67affffa671a7178180ffffa67a71173fffffff126168189867ffff269291679093739f780706602fffffff732672ffffffffff23ba68a89867ffff20727b09767a9a7f1801781a767a23bfb21b17a61671ffff896867916b63136f091b67ffffffffff7807063b0b60ffff7b6fffffffffffff76bfffffffffffff308b76ffffffffff019b76ffffffffff819831b76fffffffa126b7ffffffffff12a3086b7fffffff2902a96b7fffffff6b72a3a83a98ffff723627ffffffffff708760620fffffff276237019fffffff162186198876ffffa76a17137fffffffa7617a187108ffff03707a0a96a7ffff76a7a88a9fffffff684b86ffffffffff36b306046fffffff86b846901fffffff946963931b36ffff6846b82a1fffffff12a30b06b046ffff4b846b0292a9ffffa93a32943b36463f823842462fffffff042462ffffffffff190234246438ffff194142246fffffff8138618466a1ffffa10a06604fffffff4634386a3039a93fa946a4ffffffffff49576bffffffffff083495b76fffffff50154076bfffffffb76834354315ffff954a1276bfffffff6b712a083495ffff76b54a42a402ffff348354325a52b76f723762549fffffff954086062687ffff362376150540ffff628687218485158f954a16176137ffff16a176107870954f40a4a503a6a737af76a7a854a48affff6956b9b89fffffff36b063056095ffff0b805b01556bffff6b3635531fffffff12a95b9b8b56ffff0b306b09656912afb85b56805a52025f6b36352a3a53ffff589528562382ffff956960062fffffff158180568382628f156216ffffffffff13616a386569896fa10a06950560ffff03856affffffffffa56fffffffffffffb5a75bffffffffffb5ab75830fffffff5b75ab190fffffffa75ab7981831ffffb12b71751fffffff08312717572bffff9759279022b7ffff75272b592328982f25a235375fffffff820852875a25ffff9015a35373a2ffff982921872a25752f135375ffffffffff087071175fffffff903935537fffffff987597ffffffffff5845a8ab8fffffff5045b05abb30ffff01984a8aba45ffffab4a45b34941314f2512852b8458ffff04b0b345b2b151bf0250592b5458b85f9452b3ffffffffff25a352345384ffff5a2524420fffffff3a235a385458019f5a2524192942ffff845853351fffffff045105ffffffffff845853905035ffff945fffffffffffff4b749b9abfffffff0834979b79abffff1ab1b414074bffff3143481a474bab4f4b79b492b912ffff9749b791b2b1083fb74b42240fffffffb74b42834324ffff29a279237749ffff9a7974a27870207f37a3a274a1a040af1a2874ffffffffff491417713fffffff491417081871ffff403743ffffffffff487fffffffffffff9a8ab8ffffffffff30939bb9afffffff01a0a88abfffffff31ab3affffffffff12b1b99b8fffffff30939b1292b9ffff02b80bffffffffff32bfffffffffffff23828aa89fffffff9a2092ffffffffff23828a0181a8ffff1a2fffffffffffff138918ffffffffff091fffffffffffff038fffffffffffffffffffffffffffff";

const fn decode_triangle_table() -> [[i8; 16]; 256] {
    let bytes = TRIANGLE_NIBBLES.as_bytes();
    let mut table = [[-1_i8; 16]; 256];
    let mut index = 0;
    while index < bytes.len() {
        table[index / 16][index % 16] = match bytes[index] {
            b'0'..=b'9' => (bytes[index] - b'0') as i8,
            b'a'..=b'b' => (bytes[index] - b'a' + 10) as i8,
            b'f' => -1,
            _ => panic!("invalid marching-cubes table nibble"),
        };
        index += 1;
    }
    table
}

pub const TRIANGLE_CONNECTION_TABLE: [[i8; 16]; 256] = decode_triangle_table();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_tables_match_reference_sentinel_cases() {
        assert_eq!(CUBE_EDGE_FLAGS[0], 0x000);
        assert_eq!(CUBE_EDGE_FLAGS[1], 0x109);
        assert_eq!(CUBE_EDGE_FLAGS[127], 0x8c0);
        assert_eq!(CUBE_EDGE_FLAGS[255], 0x000);
        assert_eq!(&TRIANGLE_CONNECTION_TABLE[1][..4], &[0, 8, 3, -1]);
        assert_eq!(
            &TRIANGLE_CONNECTION_TABLE[90],
            &[0, 1, 9, 4, 7, 8, 2, 3, 11, 5, 10, 6, -1, -1, -1, -1]
        );
        assert!(TRIANGLE_CONNECTION_TABLE[255]
            .iter()
            .all(|value| *value == -1));
    }
}
