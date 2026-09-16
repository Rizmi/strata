// SPDX-License-Identifier: MIT

use std::path::Path;

use super::*;
use crate::test_support::gtk_test;

#[test]
fn abbreviate_home_shortens_user_home_path() {
    let home = glib::home_dir();
    let inside = home.join("Downloads/stuff");
    assert_eq!(
        crate::ui::settings::general::abbreviate_home(&inside),
        "~/Downloads/stuff"
    );

    let outside = Path::new("/var/log");
    assert_eq!(
        crate::ui::settings::general::abbreviate_home(outside),
        "/var/log"
    );
}

#[test]
fn search_exclusions_persist_and_update_behavior() {
    gtk_test(
        "ui::settings::exclusions::tests::search_exclusions_persist_and_update_behavior",
        || {
            ThemeManager::seed_saved_preferences_for_test();
            let manager = ThemeManager::shared();
            manager.set_search_exclusions(vec![".venv".to_owned(), "~/Secret".to_owned()]);
            assert_eq!(
                manager.search_exclusions(),
                vec![".venv".to_owned(), "~/Secret".to_owned()]
            );

            // Verify case-insensitive removal matching exclusions dialog behavior
            let mut current = manager.search_exclusions();
            current.retain(|item| !item.eq_ignore_ascii_case(".VENV"));
            manager.set_search_exclusions(current);
            assert_eq!(manager.search_exclusions(), vec!["~/Secret".to_owned()]);
        },
    );
}
