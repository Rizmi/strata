// SPDX-License-Identifier: MIT

use super::*;

#[test]
#[ignore = "requires a GTK display and isolated XDG directories; run this test alone"]
fn inline_setup_actions_follow_status_and_preserve_errors() {
    gtk::init().expect("GTK display");
    gio::resources_register_include!("strata.gresource").expect("bundled icons");
    crate::ui::prepare_portal_ui();
    for (width, orientation) in [
        (1200, gtk::Orientation::Horizontal),
        (480, gtk::Orientation::Vertical),
    ] {
        let card = settings_row();
        card.set_orientation(orientation);
        let window = gtk::Window::builder()
            .default_width(width)
            .default_height(500)
            .child(&card)
            .build();
        window.present();
        let context = glib::MainContext::default();
        let deadline = std::time::Instant::now() + Duration::from_millis(150);
        while std::time::Instant::now() < deadline {
            while context.pending() {
                context.iteration(false);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let content = card.first_child().expect("integration content");
        let title = content
            .first_child()
            .expect("card title")
            .downcast::<gtk::Label>()
            .expect("title label");
        let description = title.next_sibling().expect("description");
        let title_bounds = title.compute_bounds(&card).expect("title bounds");
        let description_bounds = description
            .compute_bounds(&card)
            .expect("description bounds");
        assert!(
            title_bounds.y() + title_bounds.height() <= description_bounds.y(),
            "title must stay above the description at width {width}"
        );
        assert!(
            title.width() >= title.layout().pixel_size().0,
            "title must not be clipped at width {width}"
        );
        window.destroy();
    }
    let parents = [
        gtk::Box::new(gtk::Orientation::Vertical, 0),
        gtk::Box::new(gtk::Orientation::Vertical, 0),
    ];
    let summaries = parents.each_ref().map(SettingsIntegrationStatus::new);
    for (chooser, folders, reveal, shortcuts, installed) in [
        (false, false, false, None, false),
        (true, false, false, Some(false), true),
        (true, true, true, Some(false), true),
        (true, true, true, Some(true), true),
        (true, true, true, None, true),
        (false, false, false, Some(false), false),
    ] {
        for summary in &summaries {
            summary.message("", false);
            let status = || {
                Ok((
                    portal_setup::PortalStatus {
                        configured: chooser,
                        has_installation: installed,
                    },
                    portal_setup::FileManagerStatus {
                        default: folders,
                        has_service: reveal,
                        has_installation: installed,
                        shortcuts,
                    },
                ))
            };
            summary.show_result(status());
            let complete = summary.complete.upgrade().expect("complete action");
            let restore = summary.restore.upgrade().expect("restore action");
            assert_eq!(
                complete.is_visible(),
                !(chooser && folders && reveal && shortcuts != Some(false))
            );
            assert_eq!(restore.is_visible(), installed);
            assert!(complete.is_sensitive());
            for (indicator, configured) in summary.indicators.iter().zip([
                Some(chooser),
                Some(folders),
                Some(reveal),
                shortcuts,
            ]) {
                let row = indicator.row.upgrade().expect("status row");
                assert_eq!(row.is_visible(), configured.is_some());
                if let Some(configured) = configured {
                    assert_eq!(
                        row.tooltip_text().as_deref(),
                        Some(
                            format!(
                                "{}: {}",
                                indicator.name,
                                if configured {
                                    "Configured"
                                } else {
                                    "Not configured"
                                }
                            )
                            .as_str()
                        )
                    );
                }
            }
            for enable in [true, false] {
                summary.show_progress(enable);
                assert!(!complete.is_sensitive());
                assert!(!restore.is_sensitive());
                assert!(
                    !summary
                        .message
                        .upgrade()
                        .expect("inline message")
                        .is_visible()
                );
                let action = if enable { &complete } else { &restore };
                assert_eq!(
                    action.label().as_deref(),
                    Some(if enable {
                        "Completing setup…"
                    } else {
                        "Restoring defaults…"
                    })
                );
                summary.show_result(status());
                assert_eq!(complete.label().as_deref(), Some("Complete setup"));
                assert_eq!(restore.label().as_deref(), Some("Restore default"));
                assert!(complete.is_sensitive());
            }
            SETUP_RUNNING.set(true);
            summary.show_result(status());
            assert!(!complete.is_sensitive());
            assert!(!restore.is_sensitive());
            SETUP_RUNNING.set(false);
            summary.message("Could not apply shortcuts", true);
            summary.show_result(status());
            let message = summary.message.upgrade().expect("inline error");
            assert!(message.is_visible());
            assert_eq!(message.text(), "Could not apply shortcuts");
            assert!(complete.is_sensitive());
            summary.show_result(Err("configuration is unreadable".into()));
            assert!(!complete.is_sensitive());
            assert!(!restore.is_sensitive());
            assert!(
                summary.indicators.iter().all(|indicator| !indicator
                    .row
                    .upgrade()
                    .expect("status row")
                    .is_visible())
            );
            summary.show_result(status());
            assert!(complete.is_sensitive());
        }
    }
}
