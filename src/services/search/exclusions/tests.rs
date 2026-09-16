// SPDX-License-Identifier: MIT

use super::*;

#[test]
fn classifies_folder_names_and_directories() {
    let raw = vec![
        ".venv".to_owned(),
        "node_modules/".to_owned(),
        "~/Downloads".to_owned(),
        "/var/log".to_owned(),
        "  build  ".to_owned(),
    ];
    let exclusions = SearchExclusions::from_strings(&raw);
    assert_eq!(
        exclusions.folder_names,
        vec![".venv", "node_modules", "build"]
    );
    assert_eq!(
        exclusions.directories,
        vec![
            glib::home_dir().join("Downloads"),
            PathBuf::from("/var/log")
        ]
    );
}

#[test]
fn matches_folder_name_anywhere_in_tree() {
    let exclusions = SearchExclusions::from_strings(&[".venv".to_owned()]);
    assert!(exclusions.is_excluded(Path::new("/home/user/project/.venv"), ".venv", true));
    assert!(exclusions.is_excluded(Path::new("/opt/app/.VENV"), ".VENV", true));
    assert!(!exclusions.is_excluded(Path::new("/home/user/project/venv"), "venv", true));
    assert!(!exclusions.is_excluded(Path::new("/home/user/project/file.txt"), "file.txt", false));
}

#[test]
fn matches_specific_directory_and_children() {
    let home = glib::home_dir();
    let target_dir = home.join("Secret");
    let exclusions = SearchExclusions::from_strings(&["~/Secret".to_owned()]);

    assert!(exclusions.is_excluded(&target_dir, "Secret", true));
    assert!(exclusions.is_excluded(&target_dir.join("sub"), "sub", true));
    assert!(exclusions.is_excluded(&target_dir.join("file.txt"), "file.txt", false));

    let other_dir = home.join("Other/Secret");
    assert!(!exclusions.is_excluded(&other_dir, "Secret", true));
}
