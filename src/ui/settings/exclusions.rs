// SPDX-License-Identifier: MIT

use std::rc::Rc;

use gtk::{gio, glib, prelude::*};

use crate::{
    assets::icons,
    services::SearchExclusions,
    ui::{
        controls::{form_entry, form_error_label, form_label, modal_layout, set_form_field_error},
        modal::{ModalHost, dismiss_modal_layer, modal_layer},
        preferences::PreferenceManager,
    },
};

#[cfg(test)]
mod tests;

pub(super) fn show_search_exclusions_dialog(
    parent: &impl IsA<gtk::Widget>,
    manager: &Rc<PreferenceManager>,
) {
    let Some(ModalHost {
        overlay: window_overlay,
        blurred_root,
    }) = ModalHost::blurred_for(parent)
    else {
        return;
    };

    let layout = modal_layout(
        icons::SEARCH,
        "Global search exclusions",
        "Folders and directories excluded from search",
        "Done",
    );
    layout.content.add_css_class("wide");

    let field_label = form_label("Exclude folder name or directory path");
    let field = form_entry();
    field.set_hexpand(true);
    field.set_placeholder_text(Some("Folder name (e.g. .venv) or ~/path…"));
    super::super::accessibility::set_label(&field, "Search exclusion");

    let browse_btn = gtk::Button::with_label("Browse…");
    browse_btn.add_css_class("action-dialog-cancel");
    browse_btn.set_valign(gtk::Align::Fill);
    browse_btn.set_size_request(84, -1);

    let add_btn = gtk::Button::with_label("Add");
    add_btn.add_css_class("action-dialog-confirm");
    add_btn.set_valign(gtk::Align::Fill);
    add_btn.set_size_request(84, -1);

    let buttons_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons_box.set_homogeneous(true);
    buttons_box.append(&browse_btn);
    buttons_box.append(&add_btn);

    let input_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    input_row.append(&field);
    input_row.append(&buttons_box);

    let error_label = form_error_label();

    layout.body.append(&field_label);
    layout.body.append(&input_row);
    layout.body.append(&error_label);

    let exclusions_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    exclusions_box.add_css_class("transfer-suggestions");

    let list_scroll = gtk::ScrolledWindow::builder()
        .child(&exclusions_box)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .min_content_height(160)
        .max_content_height(240)
        .propagate_natural_height(true)
        .build();
    list_scroll.add_css_class("transfer-suggestion-scroll");
    layout.body.append(&list_scroll);

    let weak_manager = Rc::downgrade(manager);
    manager.bind_preference(
        &exclusions_box,
        PreferenceManager::search_exclusions,
        move |widget, exclusions| {
            if let Some(manager) = weak_manager.upgrade()
                && let Some(container) = widget.downcast_ref::<gtk::Box>()
            {
                render_exclusion_rows(container, &manager, exclusions);
            }
        },
    );

    let field_for_add = field.downgrade();
    let error_for_add = error_label.downgrade();
    let manager_for_add = Rc::downgrade(manager);
    let do_add = Rc::new(move || {
        let (Some(field_for_add), Some(error_for_add), Some(manager_for_add)) = (
            field_for_add.upgrade(),
            error_for_add.upgrade(),
            manager_for_add.upgrade(),
        ) else {
            return;
        };
        let raw = field_for_add.text().to_string();
        let current = manager_for_add.search_exclusions();
        match validate_exclusion_input(&raw, &current) {
            Ok(trimmed) => {
                set_form_field_error(&field_for_add, &error_for_add, None);
                let mut updated = current;
                updated.push(trimmed);
                manager_for_add.set_search_exclusions(updated);
                field_for_add.set_text("");
            }
            Err(error) => {
                set_form_field_error(&field_for_add, &error_for_add, Some(error));
            }
        }
    });

    let do_add_activate = do_add.clone();
    field.connect_activate(move |_| {
        do_add_activate();
    });

    let error_for_change = error_label.downgrade();
    field.connect_changed(move |field| {
        if let Some(error) = error_for_change.upgrade() {
            set_form_field_error(field, &error, None);
        }
    });

    let do_add_click = do_add.clone();
    add_btn.connect_clicked(move |_| {
        do_add_click();
    });

    let field_for_browse = field.downgrade();
    let error_for_browse = error_label.downgrade();
    browse_btn.connect_clicked(move |button| {
        let Some(window) = button.root().and_downcast::<gtk::Window>() else {
            return;
        };
        let dialog = gtk::FileDialog::builder()
            .title("Select folder to exclude")
            .modal(true)
            .build();
        let field = field_for_browse.clone();
        let error = error_for_browse.clone();
        dialog.select_folder(Some(&window), gio::Cancellable::NONE, move |result| {
            let (Ok(file), Some(field), Some(error)) = (result, field.upgrade(), error.upgrade())
            else {
                return;
            };
            if let Some(path) = file.path() {
                let display = super::general::abbreviate_home(&path);
                set_form_field_error(&field, &error, None);
                field.set_text(&display);
                field.set_position(-1);
                field.grab_focus();
            }
        });
    });

    let content = layout.content;
    let close = layout.close;
    let cancel = layout.cancel;
    let confirm = layout.confirm;
    cancel.set_visible(false);

    let layer = modal_layer(&content, &window_overlay, blurred_root.clone(), None);
    window_overlay.add_overlay(&layer);

    for button in [&close, &confirm] {
        let weak_layer = layer.downgrade();
        let weak_overlay = window_overlay.downgrade();
        let weak_root = blurred_root.as_ref().map(|root| root.downgrade());
        button.connect_clicked(move |_| {
            if let (Some(layer), Some(overlay)) = (weak_layer.upgrade(), weak_overlay.upgrade()) {
                let root = weak_root.as_ref().and_then(|root| root.upgrade());
                dismiss_modal_layer(&layer, &overlay, root.as_ref());
            }
        });
    }

    let esc_confirm = confirm.downgrade();
    let key_controller = gtk::EventControllerKey::new();
    key_controller.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            if let Some(confirm) = esc_confirm.upgrade() {
                confirm.emit_clicked();
            }
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    layer.add_controller(key_controller);

    field.grab_focus();
}

fn render_exclusion_rows(
    container: &gtk::Box,
    manager: &Rc<PreferenceManager>,
    exclusions: Vec<String>,
) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
    if exclusions.is_empty() {
        let empty = gtk::Label::new(Some(
            "No custom exclusions added. Common tool caches (.venv, node_modules, target, etc.) are excluded automatically.",
        ));
        empty.add_css_class("transfer-suggestions-empty");
        empty.set_xalign(0.0);
        empty.set_wrap(true);
        container.append(&empty);
        return;
    }
    for item in exclusions {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 9);
        row.add_css_class("transfer-suggestion");
        row.append(&crate::assets::primary_icon(icons::FOLDER, 16));

        let name_label = gtk::Label::new(Some(&item));
        name_label.set_xalign(0.0);
        name_label.set_hexpand(true);
        name_label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        row.append(&name_label);

        let is_path = SearchExclusions::is_directory_path(&item);
        let type_label = gtk::Label::new(Some(if is_path { "Directory" } else { "Folder name" }));
        type_label.add_css_class("transfer-suggestion-parent");
        type_label.set_xalign(1.0);
        row.append(&type_label);

        let remove = gtk::Button::new();
        remove.add_css_class("action-dialog-close");
        remove.set_valign(gtk::Align::Center);
        remove.set_tooltip_text(Some("Remove exclusion"));
        remove.set_child(Some(&crate::assets::primary_icon(icons::X, 14)));
        super::super::accessibility::set_label(&remove, &format!("Remove exclusion {item}"));
        let weak_manager = Rc::downgrade(manager);
        remove.connect_clicked(move |_| {
            if let Some(manager) = weak_manager.upgrade() {
                let mut exclusions = manager.search_exclusions();
                exclusions.retain(|candidate| candidate != &item);
                manager.set_search_exclusions(exclusions);
            }
        });
        row.append(&remove);

        container.append(&row);
    }
}

pub(super) fn validate_exclusion_input(
    raw: &str,
    current: &[String],
) -> Result<String, &'static str> {
    let normalized = SearchExclusions::normalize_entry(raw)?;
    let is_path = SearchExclusions::is_directory_path(&normalized);
    let is_duplicate = current
        .iter()
        .filter_map(|item| SearchExclusions::normalize_entry(item).ok())
        .any(|existing| {
            if is_path {
                existing == normalized
            } else {
                existing.eq_ignore_ascii_case(&normalized)
            }
        });
    if is_duplicate {
        return Err("This exclusion has already been added.");
    }
    Ok(normalized)
}
