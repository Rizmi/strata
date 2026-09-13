// SPDX-License-Identifier: MIT

use std::collections::{HashSet, VecDeque};

use super::*;

const MAX_DIRECTORIES: usize = 4_096;
const MAX_DEPTH: usize = 16;

pub(super) fn enumerate(request: DirectoryRequest, emit: Rc<dyn Fn(DirectoryEvent)>) -> LoadHandle {
    let task = glib::MainContext::default().spawn_local(async move {
        let mut library = Library::new(&request);
        let result =
            glib::future_with_timeout(request.time_budget, library.discover(&request, &emit)).await;
        match result {
            Ok(Err(message)) => emit(DirectoryEvent::Failed {
                request_id: request.id,
                message,
            }),
            result => emit(DirectoryEvent::Finished {
                request_id: request.id,
                truncated: !matches!(result, Ok(Ok(()))) || library.truncated,
                can_trash: library.can_trash,
                can_delete: library.can_delete,
            }),
        }
    });
    LoadHandle::new(move || task.abort())
}

struct Library {
    pending: VecDeque<(Location, usize, bool)>,
    seen: HashSet<Location>,
    files: usize,
    truncated: bool,
    can_trash: Option<bool>,
    can_delete: Option<bool>,
}

impl Library {
    fn new(request: &DirectoryRequest) -> Self {
        Self {
            pending: VecDeque::from([(request.location.clone(), 0, false)]),
            seen: HashSet::from([request.location.clone()]),
            files: 0,
            truncated: false,
            can_trash: None,
            can_delete: None,
        }
    }

    async fn discover(
        &mut self,
        request: &DirectoryRequest,
        emit: &Rc<dyn Fn(DirectoryEvent)>,
    ) -> Result<(), String> {
        let attributes = if request.include_metadata {
            FULL_ATTRIBUTES
        } else {
            LIST_ATTRIBUTES
        };
        let batch_size = request.batch_size.clamp(1, 256) as i32;
        while let Some((location, depth, hidden_parent)) = self.pending.pop_front() {
            let pending_before = self.pending.len();
            let enumerator = gio_file_for_location(&location)
                .enumerate_children_future(
                    attributes,
                    gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                    glib::Priority::DEFAULT,
                )
                .await
                .map_err(|error| error.to_string())?;
            loop {
                let infos = enumerator
                    .next_files_future(batch_size, glib::Priority::DEFAULT)
                    .await
                    .map_err(|error| error.to_string())?;
                if infos.is_empty() {
                    break;
                }
                let mut entries = Vec::new();
                for info in infos {
                    let Some(child) = location.child(info.name().as_os_str()) else {
                        continue;
                    };
                    if !(child.is_within(&request.location)
                        || request.location.contains_camera_photo_location(&child))
                        || info_is_symlink(&info)
                    {
                        continue;
                    }
                    match info.file_type() {
                        gio::FileType::Directory => {
                            if depth >= MAX_DEPTH || self.seen.len() >= MAX_DIRECTORIES {
                                self.truncated = true;
                            } else if self.seen.insert(child.clone()) {
                                self.pending.push_back((
                                    child,
                                    depth + 1,
                                    hidden_parent || info_is_hidden(&info),
                                ));
                            }
                        }
                        gio::FileType::Regular => {
                            if self.files == request.max_entries {
                                self.truncated = true;
                                break;
                            }
                            if self.can_trash.is_none() {
                                self.can_trash = info_can_trash(&info);
                            }
                            if self.can_delete.is_none() {
                                self.can_delete = info_can_delete(&info);
                            }
                            // Each row retains its real URI, including duplicate basenames.
                            let mut entry = entry_from_info(child, info);
                            entry.is_hidden |= hidden_parent;
                            entries.push(entry);
                            self.files += 1;
                        }
                        _ => {}
                    }
                }
                if !entries.is_empty() {
                    emit(DirectoryEvent::Batch {
                        request_id: request.id,
                        entries,
                    });
                }
                if self.truncated && self.files == request.max_entries {
                    break;
                }
            }
            enumerator
                .close_future(glib::Priority::DEFAULT)
                .await
                .map_err(|error| error.to_string())?;
            if self.truncated && self.files == request.max_entries {
                return Ok(());
            }
            if self.pending.len() > pending_before {
                // Camera date-folder names sort chronologically; visit recent branches first.
                self.pending
                    .make_contiguous()
                    .sort_by(|left, right| right.0.compare(&left.0));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
