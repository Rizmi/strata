// SPDX-License-Identifier: MIT

use super::*;

#[test]
fn deleting_a_nested_camera_file_removes_only_its_original_uri_from_the_flat_view() {
    let browser = Browser::new(Rc::new(FakeFileSource));
    let root = Location::uri("gphoto2://camera/");
    let first = FileEntry {
        location: Location::uri("gphoto2://camera/202401_a/IMG_0001.JPG"),
        ..batch_entry("IMG_0001.JPG")
    };
    let second = FileEntry {
        location: Location::uri("gphoto2://camera/202402_a/IMG_0001.JPG"),
        ..batch_entry("IMG_0001.JPG")
    };
    {
        let mut state = browser.state.borrow_mut();
        state.navigate(root, RequestId(1));
        let _ = state.apply_batch(RequestId(1), vec![first.clone(), second.clone()]);
    }
    browser.remove_deleted_locations(&[first.location]);
    assert_eq!(
        browser.column_snapshot(0).map(|column| column.count),
        Some(1)
    );
    assert_eq!(
        browser.entry_at(0, 0).map(|entry| entry.location),
        Some(second.location)
    );
}

#[test]
fn large_camera_deletions_rescan_each_open_library_only_once() {
    let requests = Rc::new(Cell::new(0));
    let browser = Browser::new(Rc::new(RecordingFileSource {
        request_count: requests.clone(),
    }));
    browser
        .state
        .borrow_mut()
        .navigate(Location::uri("gphoto2://camera/"), RequestId(1));
    let deleted: Vec<_> = (0..=MAX_INCREMENTAL_OPERATION_UPDATES)
        .map(|i| Location::uri(format!("gphoto2://camera/folder-{i}/image.jpg")))
        .collect();
    browser.remove_deleted_locations(&deleted);
    assert_eq!(requests.get(), 1);
    let parents: Vec<_> = deleted.iter().filter_map(Location::parent).collect();
    browser.refresh_columns_at_many(&parents);
    assert_eq!(requests.get(), 2);
}

#[test]
fn operations_in_camera_subfolders_refresh_the_library_but_not_other_devices() {
    for cancelled in [false, true] {
        let requests = Rc::new(Cell::new(0));
        let browser = Browser::new(Rc::new(RecordingFileSource {
            request_count: requests.clone(),
        }));
        browser
            .state
            .borrow_mut()
            .navigate(Location::uri("gphoto2://camera/"), RequestId(1));
        for (parent, expected) in [
            ("gphoto2://other/202401_a", 0),
            ("gphoto2://camera/202401_a", 1),
        ] {
            let parent = Location::uri(parent);
            if cancelled {
                browser.refresh_after_cancellation(&HashSet::from([parent]));
            } else {
                browser.refresh_columns_at(&parent);
            }
            assert_eq!(requests.get(), expected);
        }
    }
}
