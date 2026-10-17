use super::{DesktopError, NativeReference};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "nexushub-desktop-references-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&root).expect("synthetic reference fixture directory is created");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .expect("synthetic reference fixture directory is private");
        Self(root)
    }

    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).expect("synthetic reference fixture file is written");
        path
    }

    fn sparse_file(&self, name: &str, size: u64, prefix: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, prefix).expect("synthetic sparse reference file is created");
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("synthetic sparse reference file can be extended")
            .set_len(size)
            .expect("synthetic sparse reference file size is set");
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn selected_files_are_classified_from_content_signatures() {
    let fixture = Fixture::new();
    let regular_path = fixture.file("fixture-data.bin", b"synthetic attachment contents");
    let image_path = fixture.file(
        "fixture-image.data",
        b"\x89PNG\r\n\x1a\nsynthetic image bytes",
    );

    let regular =
        NativeReference::selected_file(regular_path).expect("synthetic regular file is selected");
    let image =
        NativeReference::selected_file(image_path).expect("synthetic PNG signature is selected");

    assert_eq!(regular.kind(), "file");
    assert_eq!(image.kind(), "image");
    regular
        .validate()
        .expect("selected regular file keeps its identity");
    image.validate().expect("selected image keeps its identity");
}

#[test]
fn selected_file_detailed_returns_specific_safe_errors_and_legacy_classification() {
    let fixture = Fixture::new();
    let oversized_image = fixture.sparse_file(
        "fixture-oversized-image.png",
        20 * 1024 * 1024 + 1,
        b"\x89PNG\r\n\x1a\n",
    );
    let oversized_error = match NativeReference::selected_file_detailed(oversized_image.clone()) {
        Ok(_) => panic!("oversized synthetic image is rejected"),
        Err(error) => error,
    };
    assert_eq!(
        oversized_error,
        (
            DesktopError::InvalidInput,
            "图片超过 20MiB，请选择较小的图片"
        )
    );
    assert!(matches!(
        NativeReference::selected_file(oversized_image),
        Err(DesktopError::InvalidInput)
    ));

    let directory_error = match NativeReference::selected_file_detailed(fixture.0.clone()) {
        Ok(_) => panic!("synthetic fixture directory is rejected as a file"),
        Err(error) => error,
    };
    assert_eq!(
        directory_error,
        (DesktopError::InvalidInput, "仅支持普通文件，不能添加目录")
    );
    assert!(matches!(
        NativeReference::selected_file(fixture.0.clone()),
        Err(DesktopError::InvalidInput)
    ));

    let unavailable = fixture.0.join("fixture-disappeared.bin");
    let unavailable_error = match NativeReference::selected_file_detailed(unavailable.clone()) {
        Ok(_) => panic!("a missing synthetic file is rejected"),
        Err(error) => error,
    };
    assert_eq!(
        unavailable_error,
        (DesktopError::InvalidInput, "无法读取所选文件，请重新选择")
    );
    assert!(matches!(
        NativeReference::selected_file(unavailable),
        Err(DesktopError::InvalidInput)
    ));
}

#[test]
fn selected_symlink_is_resolved_but_replacing_its_target_expires_the_reference() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    let original = fixture.file("fixture-symlink-target.bin", b"synthetic original contents");
    let link = fixture.0.join("fixture-link.bin");
    symlink(&original, &link).expect("synthetic selected-file symlink is created");

    let reference = NativeReference::selected_file_detailed(link)
        .expect("legacy selection resolves the synthetic symlink target");
    let NativeReference::File { path, image, .. } = &reference else {
        panic!("selected symlink target remains a file reference");
    };
    assert!(!image);
    assert_eq!(
        path,
        &original.canonicalize().expect("fixture target resolves")
    );

    let replacement = fixture.file("fixture-replacement.bin", b"synthetic replacement contents");
    fs::remove_file(&original).expect("synthetic selected target is removed");
    symlink(&replacement, &original).expect("synthetic target is replaced by a symlink");
    assert_eq!(
        reference.validate().unwrap_err(),
        DesktopError::RequestExpired
    );
}

#[test]
fn app_references_reject_file_paths_and_unknown_schemes() {
    for path in [
        "file:///fixture/app",
        "https://example.invalid/app",
        "unknown:fixture",
    ] {
        assert!(matches!(
            NativeReference::app("fixture-app".into(), path.into()),
            Err(DesktopError::InvalidInput)
        ));
    }

    NativeReference::app("fixture-app".into(), "app://fixture-connector".into())
        .expect("native app connector URI is accepted")
        .validate()
        .expect("native app connector URI remains valid");
}

#[test]
fn captured_skill_rejects_truncation_and_symlink_replacement() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();

    let truncated_path = fixture.file(
        "truncated-skill.md",
        b"synthetic skill body before truncation",
    );
    let captured_truncated =
        NativeReference::skill("fixture-truncated-skill".into(), truncated_path.clone())
            .expect("synthetic skill reference is accepted")
            .capture()
            .expect("synthetic skill identity is captured");
    fs::write(&truncated_path, b"x").expect("synthetic skill fixture is truncated");
    assert_eq!(
        captured_truncated.validate().unwrap_err(),
        DesktopError::RequestExpired
    );

    let replaced_path = fixture.file("replaced-skill.md", b"synthetic original skill body");
    let replacement_target = fixture.file("replacement-target.md", b"synthetic replacement body");
    let captured_replaced =
        NativeReference::skill("fixture-replaced-skill".into(), replaced_path.clone())
            .expect("synthetic skill reference is accepted")
            .capture()
            .expect("synthetic skill identity is captured");
    fs::remove_file(&replaced_path).expect("captured skill fixture is removed");
    symlink(&replacement_target, &replaced_path)
        .expect("captured path is replaced with a fixture symlink");
    assert_eq!(
        captured_replaced.validate().unwrap_err(),
        DesktopError::RequestExpired
    );
}
