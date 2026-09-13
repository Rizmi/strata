// SPDX-License-Identifier: MIT

use super::*;

#[test]
fn camera_thumbnail_bindings_keep_duplicate_names_distinct_and_discard_stale_results() {
    crate::test_support::gtk_test(
        "ui::thumbnail::camera::tests::camera_thumbnail_bindings_keep_duplicate_names_distinct_and_discard_stale_results",
        || {
            crate::ui::theme::ThemeManager::shared();
            clear_thumbnail_runtime();
            hold_thumbnail_workers();
            let images = [ThumbnailSlot::new(64), ThumbnailSlot::new(64)];
            let paths = [
                "gphoto2://camera/202606/IMG_0001.JPG",
                "gphoto2://camera/202605/IMG_0001.JPG",
            ];
            for (image, uri) in images.iter().zip(paths) {
                let entry = FileEntry {
                    location: crate::model::Location::uri(uri),
                    thumbnail_path: None,
                    native_name: "IMG_0001.JPG".into(),
                    display_name: "IMG_0001.JPG".into(),
                    kind: crate::model::EntryKind::File,
                    size: MetadataValue::Unknown,
                    modified_unix_seconds: MetadataValue::Unknown,
                    mode: MetadataValue::Unavailable,
                    is_hidden: false,
                };
                set_thumbnail_or_icon(image, &entry, crate::assets::icons::PICTURES, 32, 64);
            }
            while glib::MainContext::default().iteration(false) {}
            fire_settled_thumbnails();
            let results = PENDING_THUMBNAILS.with(|pending| {
                let pending = pending.borrow();
                assert_eq!(
                    pending.len(),
                    2,
                    "queued keys: {:?}; active {}",
                    pending.keys().map(|key| &key.path).collect::<Vec<_>>(),
                    ACTIVE_REQUESTS.with(|requests| requests.borrow().len())
                );
                paths.map(|uri| {
                    let (key, request) = pending
                        .iter()
                        .find(|(key, _)| key.path == Path::new(uri))
                        .expect("distinct camera job without waiting for metadata");
                    assert_eq!(request.kind, ThumbnailKind::Camera);
                    (key.clone(), request.id)
                })
            });
            let stale = take_pending_targets(&results[0].0, results[0].1).expect("first targets");
            let live = take_pending_targets(&results[1].0, results[1].1).expect("second targets");
            show_fallback_icon(&images[0], crate::assets::icons::FOLDER, 32);
            let pixels = glib::Bytes::from_owned(vec![255u8; 4]);
            let texture = gdk::MemoryTexture::new(1, 1, gdk::MemoryFormat::R8g8b8a8, &pixels, 4)
                .upcast::<gdk::Texture>();
            finish_thumbnail_targets(stale, Some(&texture), Path::new(paths[0]));
            finish_thumbnail_targets(live, Some(&texture), Path::new(paths[1]));
            assert!(!displayed_thumbnail_matches(
                &images[0],
                Path::new(paths[0])
            ));
            assert!(displayed_thumbnail_matches(&images[1], Path::new(paths[1])));
            clear_thumbnail_runtime();
        },
    );
}

#[test]
fn camera_icon_reads_are_bounded_even_without_size_metadata() {
    let _lock = crate::test_support::ASYNC_MAIN_CONTEXT_DEFAULT
        .lock()
        .expect("context lock");
    let context = glib::MainContext::default();
    let _owner = context.acquire().expect("context owner");
    for (size, limit, succeeds) in [(0, 0, true), (8193, 8193, true), (8193, 8192, false)] {
        let input = vec![7; size];
        let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from_owned(input.clone()));
        let result = context.block_on(read_icon(&stream, limit));
        assert_eq!(result.is_ok(), succeeds);
        if succeeds {
            assert_eq!(result.expect("icon bytes"), input);
        }
    }
}
