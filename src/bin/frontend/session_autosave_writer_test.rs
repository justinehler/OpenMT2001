//! Fixed-slot and failure-preservation regressions.
use super::*;
struct Scratch(PathBuf);
impl Scratch {
    fn new()->Self {
        let base=std::env::var_os("OPENMT2001_TEST_SCRATCH").map(PathBuf::from)
            .unwrap_or_else(||PathBuf::from("work/test-scratch"));
        fs::create_dir_all(&base).unwrap();
        let path=base.join(format!("autosave-{}-{}",std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&path).unwrap();Self(path)
    }
}
impl Drop for Scratch {fn drop(&mut self){fs::remove_dir_all(&self.0).unwrap();}}
fn native(cash:u32)->Vec<u8> {
    let mut bytes=Vec::new();
    for value in [0x2000u32,16,7,12,cash,3,u32::MAX,0] {bytes.extend_from_slice(&value.to_le_bytes());}
    bytes
}
#[test]
fn fixed_autosave_first_replace_and_profiles_are_exact() {
    let scratch=Scratch::new();let a=selected_profile_root(&scratch.0,b"Alice");
    let b=selected_profile_root(&scratch.0,b"Bob");
    let path=PreparedAutosave::prepare(&a).unwrap().publish(native(10)).unwrap();
    assert_eq!(path,fs::canonicalize(&a).unwrap().join("saves").join("autosave.sav"));
    assert_eq!(PreparedAutosave::prepare(&a).unwrap().publish(native(30)).unwrap(),path);
    PreparedAutosave::prepare(&b).unwrap().publish(native(20)).unwrap();
    assert_eq!(fs::read(path).unwrap(),native(30));
    assert_eq!(fs::read(b.join("saves/autosave.sav")).unwrap(),native(20));
    assert_eq!(list_slots(&a).unwrap().iter().map(|slot|slot.id.as_str()).collect::<Vec<_>>(),["autosave.sav"]);
}
#[test]
fn encoding_write_and_publish_failures_preserve_prior_without_permission_changes() {
    let scratch=Scratch::new();let root=scratch.0.join("profile");
    let path=PreparedAutosave::prepare(&root).unwrap().publish(native(17)).unwrap();
    assert!(PreparedAutosave::prepare(&root).unwrap().publish(vec![1,2,3]).is_err());
    let mut prepared=PreparedAutosave::prepare(&root).unwrap();
    // A real read-only handle gives write_all an I/O error; no chmod/elevation.
    prepared.file=Some(fs::File::open(&prepared.temporary).unwrap());
    assert!(prepared.publish(native(31)).is_err());
    assert_eq!(fs::read(&path).unwrap(),native(17));
    let mut prepared=PreparedAutosave::prepare(&root).unwrap();
    let actual_temporary=prepared.temporary.clone();
    prepared.temporary=root.join("saves/nonexistent-owned-stage");
    assert!(prepared.publish(native(47)).is_err());
    fs::remove_file(actual_temporary).unwrap();
    assert_eq!(fs::read(&path).unwrap(),native(17));
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(),1);
}
#[test]
fn abandoned_capture_and_failed_write_allow_retry_in_the_same_fixed_slot() {
    let scratch=Scratch::new();let root=scratch.0.join("profile");
    let prior=native(17);
    let path=PreparedAutosave::prepare(&root).unwrap().publish(prior.clone()).unwrap();
    let directory=path.parent().unwrap();

    // The host prepares storage before asking the camera/encoder. An early
    // unsupported-state return drops this request without calling publish.
    let prepared=PreparedAutosave::prepare(&root).unwrap();
    let stage=prepared.temporary.clone();
    assert!(stage.is_file());
    drop(prepared);
    assert!(!stage.exists());
    assert_eq!(fs::read(&path).unwrap(),prior);

    // Exercise an actual write error, then a fresh request in this process.
    // Filesystem contents, rather than the codec, are the recovery oracle.
    let mut prepared=PreparedAutosave::prepare(&root).unwrap();
    let stage=prepared.temporary.clone();
    prepared.file=Some(fs::File::open(&stage).unwrap());
    assert!(prepared.publish(native(31)).is_err());
    assert!(!stage.exists());
    assert_eq!(fs::read(&path).unwrap(),prior);
    assert_eq!(fs::read_dir(directory).unwrap().count(),1);

    assert_eq!(PreparedAutosave::prepare(&root).unwrap().publish(native(47)).unwrap(),path);
    assert_eq!(fs::read(&path).unwrap(),native(47));
    assert_eq!(fs::read_dir(directory).unwrap().count(),1);
}
#[test]
fn fixed_slot_refuses_directory_case_collision_and_changed_slot() {
    let scratch=Scratch::new();let root=scratch.0.join("profile");
    let directory=writable_directory(&root).unwrap();
    fs::create_dir(directory.join("autosave.sav")).unwrap();
    assert!(PreparedAutosave::prepare(&root).is_err());
    fs::remove_dir(directory.join("autosave.sav")).unwrap();
    fs::write(directory.join("AutoSave.sav"),native(1)).unwrap();
    assert!(PreparedAutosave::prepare(&root).is_err());
    fs::remove_file(directory.join("AutoSave.sav")).unwrap();
    let prepared=PreparedAutosave::prepare(&root).unwrap();
    fs::write(directory.join("autosave.sav"),native(29)).unwrap();
    assert!(prepared.publish(native(41)).is_err());
    assert_eq!(fs::read(directory.join("autosave.sav")).unwrap(),native(29));
}
#[cfg(unix)]
#[test]
fn fixed_slot_refuses_symlink_without_touching_target() {
    let scratch=Scratch::new();let root=scratch.0.join("profile");
    let directory=writable_directory(&root).unwrap();let target=scratch.0.join("untouched.sav");
    fs::write(&target,native(53)).unwrap();
    std::os::unix::fs::symlink(&target,directory.join("autosave.sav")).unwrap();
    assert!(PreparedAutosave::prepare(&root).is_err());
    assert_eq!(fs::read(target).unwrap(),native(53));
}
