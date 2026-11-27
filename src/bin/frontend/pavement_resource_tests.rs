//! Source regressions; all embedded bytes below are authored fixtures.
//! No originals, real native manager handle, game or render acceptance is claimed.
use super::*;
use openmt2001::mesh::CachedMesh;

fn authored_archive() -> (ArchiveIndex, Vec<u8>) {
    // A minimal valid cached mesh, two vertices/one frame/no parts or textures.
    // A real ArchiveIndex entry points into a prefixed/suffixed payload.
    let mut mesh = vec![0u8; 164 + 2 * 12 + 2 * 8 + 2 * 12];
    mesh[0..2].copy_from_slice(&2u16.to_le_bytes());
    mesh[4..6].copy_from_slice(&1u16.to_le_bytes());
    for (i, f) in [-3f32, 2., -7., 9., 6., 11.].into_iter().enumerate() {
        mesh[164 + i * 4..168 + i * 4].copy_from_slice(&f.to_le_bytes());
    }
    let offset = 13usize;
    let length = mesh.len();
    let mut archive = vec![0x5a; offset];
    archive.extend_from_slice(&mesh);
    archive.extend_from_slice(&[0xa5; 9]);
    let mut metadata = [0u32; 13];
    metadata[1] = offset as u32;
    metadata[2] = length as u32;
    metadata[5] = 0x1234;
    let mut header_words = [0u32; 8];
    header_words[0] = 3;
    header_words[1] = 1;
    header_words[2] = 0x5678;
    let index = ArchiveIndex {
        header_words,
        version: 3,
        header_flags: 0x5678,
        entries: vec![ArchiveEntry {
            path: PATH.into(),
            offset: offset as u64,
            length: length as u64,
            metadata,
        }],
    };
    (index, archive)
}
fn loaded(index: &ArchiveIndex, archive: &[u8]) -> (LoadedPavement, [u8; 116]) {
    // Mirrors the single existing selection load/definition computation.
    let e = index.find(PATH).unwrap();
    let mesh =
        CachedMesh::parse(&archive[e.offset as usize..(e.offset + e.length) as usize]).unwrap();
    let definition = mesh.model_definition().unwrap();
    (
        LoadedPavement::from_loaded(index, archive, e, definition).unwrap(),
        definition,
    )
}
#[test]
fn successful_loaded_entry_owns_definition_provenance_and_exact_source() {
    let (index, mut archive) = authored_archive();
    let (entry, definition) = loaded(&index, &archive);
    let owner = Directory::install(entry).unwrap();
    let binding = owner.binding().unwrap();
    assert_eq!(owner.resolve(&binding).unwrap(), definition);
    assert_eq!(binding.definition().unwrap(), definition);
    assert_eq!(owner.reference_role(), ReferenceRole::NativeStatic688b48);
    assert_eq!(owner.populated_slots().unwrap(), 1); // outcome of actual finite registration
    let Provenance::Archive(provenance) = owner.provenance().unwrap() else {
        panic!("archive provenance lost")
    };
    assert_eq!(provenance.header_words, index.header_words);
    assert_eq!(provenance.entry_index, 0);
    assert_eq!(provenance.entry, index.entries[0]);
    assert_eq!(provenance.archive_length, archive.len() as u64);
    let e = &index.entries[0];
    let original = archive[e.offset as usize..(e.offset + e.length) as usize].to_vec();
    assert_eq!(owner.retained_source_length().unwrap(), original.len());
    archive.fill(0);
    drop(archive);
    drop(index);
    let state = owner.state.lock().unwrap();
    assert_eq!(
        &*state.entry(binding.handle()).unwrap().resource.source_bytes,
        original.as_slice()
    );
    drop(state);
    assert_eq!(binding.definition().unwrap(), definition);
}
#[test]
fn bind_aliases_same_generation_but_actual_replacement_revokes_equal_definition() {
    let (index, archive) = authored_archive();
    let (resource, definition) = loaded(&index, &archive);
    let mut owner = Directory::install(resource).unwrap();
    let old = owner.binding().unwrap();
    let alias = owner.binding().unwrap();
    assert_eq!(old.handle(), alias.handle());
    let (replacement, equal) = loaded(&index, &archive);
    assert_eq!(equal, definition);
    owner.replace(replacement).unwrap();
    let fresh = owner.binding().unwrap();
    assert_ne!(old.handle(), fresh.handle());
    assert_eq!(fresh.definition().unwrap(), definition);
    assert!(old.definition().is_err());
    assert!(alias.definition().is_err());
    assert!(owner.resolve(&old).is_err());
    assert_eq!(owner.populated_slots().unwrap(), 1);
}
#[test]
fn equal_definition_other_directory_and_forged_local_slot_cannot_resolve() {
    let (index, archive) = authored_archive();
    let a = Directory::install(loaded(&index, &archive).0).unwrap();
    let b = Directory::install(loaded(&index, &archive).0).unwrap();
    let binding = a.binding().unwrap();
    assert_ne!(binding.handle(), b.binding().unwrap().handle());
    assert!(b.resolve(&binding).is_err());
    // Private corruption available only INSIDE this module proves a caller
    // knowing the other local handle still cannot forge its occupied binding.
    let forged = ResolvedBinding {
        handle: b.binding().unwrap().handle(),
        directory: binding.directory.clone(),
    };
    assert!(forged.definition().is_err());
    assert!(b.resolve(&forged).is_err());
    assert!(a.resolve(&forged).is_err());
}
#[test]
fn retirement_repopulation_drop_and_exhaustion_never_revive_old_aliases() {
    let (index, archive) = authored_archive();
    let mut owner = Directory::install(loaded(&index, &archive).0).unwrap();
    let old = owner.binding().unwrap();
    owner.retire().unwrap();
    assert_eq!(owner.populated_slots().unwrap(), 0);
    assert!(owner.binding().is_err());
    assert!(owner.resolve(&old).is_err());
    assert!(old.definition().is_err());
    owner.replace(loaded(&index, &archive).0).unwrap();
    let fresh = owner.binding().unwrap();
    assert_ne!(old.handle(), fresh.handle());
    assert!(old.definition().is_err());
    drop(owner);
    assert!(fresh.definition().is_err());
    let mut owner = Directory::install(loaded(&index, &archive).0).unwrap();
    let old = owner.binding().unwrap();
    owner.state.lock().unwrap().generation = Generation(u64::MAX);
    assert!(owner.replace(loaded(&index, &archive).0).is_err());
    assert_eq!(owner.populated_slots().unwrap(), 0);
    assert!(owner.binding().is_err());
    assert!(old.definition().is_err());
    assert!(owner.populate(loaded(&index, &archive).0).is_err());
}
#[test]
fn install_rejects_detached_archive_entry_and_out_of_bounds_source() {
    let (index, archive) = authored_archive();
    let e = index.find(PATH).unwrap();
    let copied = e.clone();
    assert!(LoadedPavement::from_loaded(&index, &archive, &copied, [0; 116]).is_err());
    assert!(LoadedPavement::from_loaded(&index, &[], e, [0; 116]).is_err());
}
