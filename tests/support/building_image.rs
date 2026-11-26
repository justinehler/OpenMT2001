// Authored inert PE headers/data for independent BuildingKind tests.
pub fn fixture() -> Vec<u8> {
    const START: u32 = 0x525000;
    const SIZE: usize = 0x11800;
    let mut bytes = vec![0; 0x200 + SIZE];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
    bytes[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
    bytes[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
    bytes[0x94..0x96].copy_from_slice(&32u16.to_le_bytes());
    bytes[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
    bytes[0xb4..0xb8].copy_from_slice(&0x400000u32.to_le_bytes());
    bytes[0xc4..0xc8].copy_from_slice(&(START - 0x400000).to_le_bytes());
    bytes[0xc8..0xcc].copy_from_slice(&(SIZE as u32).to_le_bytes());
    bytes[0xcc..0xd0].copy_from_slice(&0x200u32.to_le_bytes());
    let mut put = |address: u32, value: &[u8]| {
        let at = 0x200 + (address - START) as usize;
        bytes[at..at + value.len()].copy_from_slice(value);
    };
    for (address, value) in [
        (0x52502c, 0.5f32),
        (0x525034, 1.0),
        (0x525098, 35.0),
        (0x5250a0, 10.0),
        (0x525b10, 1.2),
        (0x5251e8, 1.5),
    ] {
        put(address, &value.to_le_bytes());
    }
    for (address, value) in [(0x525b00, 0.8f64), (0x525b08, 0.6), (0x525c40, 1.0)] {
        put(address, &value.to_le_bytes());
    }
    for (kind, name) in [b"RETAIL".as_slice(), b"RESIDENTIAL", b"LANDSCAPE"]
        .into_iter()
        .enumerate()
    {
        let address = 0x5365d0 + kind as u32 * 46;
        put(address, name);
        put(address + 32, &10u16.to_le_bytes());
        put(address + 34, &10u16.to_le_bytes());
        // Original unconfigured demolition word is zero.
    }
    bytes
}
