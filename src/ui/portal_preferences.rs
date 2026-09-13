// SPDX-License-Identifier: MIT

use std::{cell::Cell, rc::Rc, time::Duration};

use gtk::{gio, glib, prelude::*};

use super::{
    blur::BlurBin,
    browser::{dismiss_modal_layer, modal_layer},
    controls::{ModalLayout, ModalTone, message_dialog_layout},
};
use crate::{assets::icons, portal_setup};

#[cfg(test)]
mod tests;

thread_local! {
    static OFFER_SCHEDULED: Cell<bool> = const { Cell::new(false) };
    static SETUP_RUNNING: Cell<bool> = const { Cell::new(false) };
}

pub(super) fn schedule_offer(window: &gtk::ApplicationWindow) {
    if OFFER_SCHEDULED.replace(true) {
        return;
    }
    let window = window.downgrade();
    glib::timeout_add_local(Duration::from_secs(2), move || {
        let Some(window) = window.upgrade() else {
            OFFER_SCHEDULED.set(false);
            return glib::ControlFlow::Break;
        };
        if !window.is_active() || super::window::visible_modal_layer(&window).is_some() {
            return glib::ControlFlow::Continue;
        }
        let window = window.downgrade();
        glib::spawn_future_local(async move {
            match gio::spawn_blocking(portal_setup::take_prompt_offer).await {
                Ok(Ok(true)) => {
                    if let Some(window) = window.upgrade() {
                        show_dialog(window.upcast_ref(), true);
                    }
                }
                Ok(Err(error)) => {
                    tracing::warn!(%error, "Could not offer file chooser integration")
                }
                Err(error) => tracing::warn!(?error, "File chooser offer task failed"),
                Ok(Ok(false)) => {}
            }
        });
        glib::ControlFlow::Break
    });
}

pub(super) fn settings_row() -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    row.add_css_class("settings-option");
    let copy = gtk::Box::new(gtk::Orientation::Vertical, 2);
    copy.set_hexpand(true);
    let title = gtk::Label::new(Some("System file chooser"));
    title.set_xalign(0.0);
    title.add_css_class("settings-option-title");
    let description = gtk::Label::new(Some(
        "Make Strata your default file manager: Open and Save dialogs, opening folders, and Reveal in File Manager from other apps.",
    ));
    description.set_xalign(0.0);
    description.set_wrap(true);
    description.add_css_class("settings-option-description");
    let configure = gtk::Button::with_label("Configure…");
    configure.add_css_class("action-dialog-cancel");
    configure.set_halign(gtk::Align::Start);
    configure.set_valign(gtk::Align::Center);
    configure.connect_clicked(|button| {
        if let Some(window) = button.root().and_downcast::<gtk::Window>() {
            show_dialog(&window, false);
        }
    });
    copy.append(&title);
    copy.append(&description);
    row.append(&copy);
    row.append(&configure);
    row
}

struct Dialog {
    layer: glib::WeakRef<gtk::Box>,
    overlay: glib::WeakRef<gtk::Overlay>,
    root: Option<glib::WeakRef<BlurBin>>,
    confirm: glib::WeakRef<gtk::Button>,
    cancel: glib::WeakRef<gtk::Button>,
    close: glib::WeakRef<gtk::Button>,
    status: glib::WeakRef<gtk::Label>,
    description: glib::WeakRef<gtk::Label>,
    success: glib::WeakRef<gtk::Image>,
    loading: glib::WeakRef<gtk::Spinner>,
    busy: Rc<Cell<bool>>,
    enable: Cell<Option<bool>>,
    finished: Cell<bool>,
}

impl Dialog {
    fn dismiss(&self) {
        if !self.busy.get()
            && let (Some(layer), Some(overlay)) = (self.layer.upgrade(), self.overlay.upgrade())
        {
            let root = self.root.as_ref().and_then(glib::WeakRef::upgrade);
            dismiss_modal_layer(&layer, &overlay, root.as_ref());
        }
    }

    fn set_busy(&self, busy: bool) {
        self.busy.set(busy);
        for button in [&self.confirm, &self.cancel, &self.close] {
            if let Some(button) = button.upgrade() {
                button.set_sensitive(!busy);
            }
        }
        if let Some(loading) = self.loading.upgrade() {
            loading.set_visible(busy);
            loading.set_spinning(busy);
        }
    }

    fn message(&self, message: &str, error: bool) {
        if let Some(status) = self.status.upgrade() {
            status.set_text(
                &message
                    .lines()
                    .map(|line| {
                        super::controls::wrap_dialog_text(
                            line,
                            super::controls::MESSAGE_DIALOG_WIDTH_CHARS,
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
            if error {
                status.add_css_class("error");
            } else {
                status.remove_css_class("error");
            }
        }
    }

    fn complete(&self, message: &str) {
        self.finished.set(true);
        self.message(message, false);
        if let Some(description) = self.description.upgrade() {
            description.set_visible(false);
        }
        if let Some(success) = self.success.upgrade() {
            success.set_visible(true);
        }
        if let Some(status) = self.status.upgrade() {
            status.set_xalign(0.5);
            status.set_justify(gtk::Justification::Center);
        }
        if let Some(confirm) = self.confirm.upgrade() {
            confirm.set_label("Done");
            confirm.grab_focus();
        }
        if let Some(cancel) = self.cancel.upgrade() {
            cancel.set_visible(false);
        }
    }

    fn load(self: &Rc<Self>) {
        self.set_busy(true);
        let dialog = self.clone();
        glib::spawn_future_local(async move {
            let result = gio::spawn_blocking(|| {
                let _ = portal_setup::dismiss_prompt();
                let chooser = portal_setup::status()?;
                let file_manager = portal_setup::file_manager_status()?;
                let configured =
                    chooser.configured && file_manager.default && file_manager.has_service;
                let has_installation =
                    chooser.has_installation || file_manager.has_service || file_manager.default;
                Ok::<_, String>((configured, has_installation))
            })
            .await;
            dialog.set_busy(false);
            match result {
                Ok(Ok((configured, has_installation))) => {
                    dialog.enable.set(Some(!configured));
                    dialog.message(
                        if configured {
                            "Strata is currently your default file manager. Open and Save dialogs, opening folders, and Reveal in File Manager from other apps all use Strata. Restoring removes this integration."
                        } else if has_installation {
                            "Some Strata integration exists, but it is not fully configured. You can complete the setup below."
                        } else {
                            "Your current file manager and chooser have not been changed. Enabling Strata makes it the default for Open and Save dialogs, opening folders, and Reveal in File Manager from other apps."
                        },
                        false,
                    );
                    if let Some(confirm) = dialog.confirm.upgrade() {
                        confirm.set_label(if configured {
                            "Restore previous"
                        } else {
                            "Use Strata"
                        });
                    }
                }
                Ok(Err(error)) => dialog.load_failed(&error),
                Err(error) => {
                    dialog.load_failed(&format!("Could not read configuration: {error:?}"))
                }
            }
            if let Some(cancel) = dialog.cancel.upgrade() {
                cancel.grab_focus();
            }
        });
    }

    fn load_failed(&self, error: &str) {
        self.message(error, true);
        if let Some(confirm) = self.confirm.upgrade() {
            confirm.set_label("Retry");
        }
    }

    fn apply(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        }
        if self.finished.get() {
            self.dismiss();
            return;
        }
        let Some(enable) = self.enable.get() else {
            self.load();
            return;
        };
        if SETUP_RUNNING.replace(true) {
            self.message(
                "Another configuration is running. Try again when it finishes.",
                true,
            );
            return;
        }
        self.set_busy(true);
        self.message(
            if enable {
                "Configuring Strata as your default file manager…"
            } else {
                "Restoring your previous file manager and chooser…"
            },
            false,
        );
        let hold = self
            .confirm
            .upgrade()
            .and_then(|button| button.root().and_downcast::<gtk::Window>())
            .and_then(|window| window.application())
            .map(|application| application.hold());
        let dialog = self.clone();
        glib::spawn_future_local(async move {
            let _hold = hold;
            let result = gio::spawn_blocking(move || {
                let outcome: Result<String, String> = if enable {
                    let chooser = portal_setup::install()?;
                    let file_manager = portal_setup::install_file_manager()?;
                    Ok(format!("{chooser}\n{file_manager}"))
                } else {
                    let chooser = portal_setup::uninstall()?;
                    let file_manager = portal_setup::uninstall_file_manager()?;
                    Ok(format!("{chooser}\n{file_manager}"))
                };
                outcome
            })
            .await;
            SETUP_RUNNING.set(false);
            dialog.set_busy(false);
            match result {
                Ok(Ok(message)) => dialog.complete(&message),
                Ok(Err(error)) => dialog.message(&error, true),
                Err(error) => dialog.message(&format!("Setup failed: {error:?}"), true),
            }
        });
    }
}

fn show_dialog(parent: &gtk::Window, offer: bool) {
    if let Some(dialog) = build_dialog(parent, offer) {
        dialog.load();
    }
}

fn build_dialog(parent: &gtk::Window, offer: bool) -> Option<Rc<Dialog>> {
    let overlay = parent.child().and_downcast::<gtk::Overlay>()?;
    let root = overlay.child().and_downcast::<BlurBin>();
    let heading = if offer {
        "Use Strata as your file manager?"
    } else {
        "System file manager"
    };
    let layout: ModalLayout = message_dialog_layout(
        icons::FOLDER,
        heading,
        "Open and Save dialogs, opening folders, and Reveal in File Manager from other apps.",
        "Use Strata",
        ModalTone::Accent,
    );
    layout.content.add_css_class("portal-setup-dialog");
    layout
        .cancel
        .set_label(if offer { "Not now" } else { "Cancel" });
    let description = gtk::Label::new(Some(&super::controls::wrap_dialog_text(
        "This sets Strata as the default for Open and Save dialogs (FileChooser portal), opening folders (inode/directory), and Reveal in File Manager (FileManager1 D-Bus). Requires xdg-desktop-portal and xdg-mime. Changing this restarts the portal service and reloads the D-Bus session bus.",
        super::controls::MESSAGE_DIALOG_WIDTH_CHARS,
    )));
    description.set_xalign(0.0);
    description.set_wrap(true);
    description.set_max_width_chars(super::controls::MESSAGE_DIALOG_WIDTH_CHARS as i32);
    description.add_css_class("settings-option-description");
    layout.body.append(&description);
    let success = crate::assets::primary_icon(icons::CIRCLE_CHECK, 48);
    success.set_halign(gtk::Align::Center);
    success.set_visible(false);
    layout.body.append(&success);
    let status = gtk::Label::new(Some("Checking your current configuration…"));
    status.set_xalign(0.0);
    status.set_wrap(true);
    status.set_max_width_chars(super::controls::MESSAGE_DIALOG_WIDTH_CHARS as i32);
    status.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    status.add_css_class("form-message");
    layout.body.append(&status);
    let busy = Rc::new(Cell::new(false));
    let blocked = busy.clone();
    let layer = modal_layer(
        &layout.content,
        &overlay,
        root.clone(),
        Some(Rc::new(move || blocked.get())),
    );
    let dialog = Rc::new(Dialog {
        layer: layer.downgrade(),
        overlay: overlay.downgrade(),
        root: root.as_ref().map(ObjectExt::downgrade),
        confirm: layout.confirm.downgrade(),
        cancel: layout.cancel.downgrade(),
        close: layout.close.downgrade(),
        status: status.downgrade(),
        description: description.downgrade(),
        success: success.downgrade(),
        loading: layout.loading.downgrade(),
        busy,
        enable: Cell::new(None),
        finished: Cell::new(false),
    });
    for button in [&layout.cancel, &layout.close] {
        let dialog = dialog.clone();
        button.connect_clicked(move |_| dialog.dismiss());
    }
    let action = dialog.clone();
    layout.confirm.connect_clicked(move |_| action.apply());
    let escaped = dialog.clone();
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            escaped.dismiss();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    layer.add_controller(keys);
    if let Some(root) = root {
        root.set_blurred(true);
    }
    overlay.add_overlay(&layer);
    Some(dialog)
}
