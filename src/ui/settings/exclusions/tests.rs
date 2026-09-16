// SPDX-License-Identifier: MIT

use super::*;
use crate::test_support::gtk_test;

#[test]
fn format_display_path_abbreviates_home() {
    let home = std::env::var_os("HOME").map(PathBuf::from).expect("HOME");
    let inside = home.join("Downloads/stuff");
    assert_eq!(format_display_path(&inside), "~/Downloads/stuff");

    let outside = Path::new("/var/log");
    assert_eq!(format_display_path(outside), "/var/log");
}

#[test]
fn render_exclusion_rows_updates_list_and_remove_action() {
    gtk_test(
        "ui::settings::exclusions::tests::render_exclusion_rows_updates_list_and_remove_action",
        || {
            ThemeManager::seed_saved_preferences_for_test();
            let manager = ThemeManager::shared();
            manager
                .set_search_exclusions(vec![".venv".to_owned(), "~/Documents/Secret".to_owned()]);

            let container = gtk::Box::new(gtk::Orientation::Vertical, 2);
            let removed = Rc::new(RefCell::new(Vec::new()));
            let removed_clone = removed.clone();
            let on_remove = Rc::new(move |item: &str| {
                removed_clone.borrow_mut().push(item.to_owned());
            });

            render_exclusion_rows(&container, &manager, on_remove);

            let mut count = 0;
            let mut child = container.first_child();
            while let Some(widget) = child {
                count += 1;
                child = widget.next_sibling();
            }
            assert_eq!(count, 2);

            // Trigger remove on the first row
            let first_row = container.first_child().expect("first row");
            let mut remove_btn = None;
            let mut row_child = first_row.first_child();
            while let Some(widget) = row_child {
                if let Ok(btn) = widget.clone().downcast::<gtk::Button>() {
                    remove_btn = Some(btn);
                }
                row_child = widget.next_sibling();
            }
            let remove_btn = remove_btn.expect("remove button");
            remove_btn.emit_clicked();

            assert_eq!(*removed.borrow(), vec![".venv".to_owned()]);
        },
    );
}

#[test]
fn render_exclusion_rows_shows_empty_message_when_no_exclusions() {
    gtk_test(
        "ui::settings::exclusions::tests::render_exclusion_rows_shows_empty_message_when_no_exclusions",
        || {
            ThemeManager::seed_saved_preferences_for_test();
            let manager = ThemeManager::shared();
            manager.set_search_exclusions(Vec::new());

            let container = gtk::Box::new(gtk::Orientation::Vertical, 2);
            let on_remove = Rc::new(|_: &str| {});
            render_exclusion_rows(&container, &manager, on_remove);

            let first_child = container.first_child().expect("empty message");
            let label = first_child.downcast::<gtk::Label>().expect("empty label");
            assert!(label.has_css_class("transfer-suggestions-empty"));
        },
    );
}
