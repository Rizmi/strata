// SPDX-License-Identifier: MIT

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct SearchExclusions {
    pub folder_names: Vec<String>,
    pub directories: Vec<PathBuf>,
}

impl SearchExclusions {
    pub fn from_strings(raw: &[String]) -> Self {
        let mut folder_names = Vec::new();
        let mut directories = Vec::new();
        let home = glib::home_dir();
        for item in raw {
            let trimmed = item.trim().trim_end_matches(['/', '\\']);
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with('~')
                || trimmed.starts_with('/')
                || trimmed.contains('/')
                || trimmed.contains('\\')
            {
                let resolved = if trimmed == "~" {
                    home.clone()
                } else if let Some(rest) = trimmed.strip_prefix("~/") {
                    home.join(rest)
                } else {
                    PathBuf::from(trimmed)
                };
                directories.push(resolved);
            } else {
                folder_names.push(trimmed.to_owned());
            }
        }
        Self {
            folder_names,
            directories,
        }
    }

    pub fn is_excluded(&self, path: &Path, name: &str, is_directory: bool) -> bool {
        if is_directory {
            if self
                .folder_names
                .iter()
                .any(|folder| name.eq_ignore_ascii_case(folder))
            {
                return true;
            }
            if self
                .directories
                .iter()
                .any(|dir| path == dir || path.starts_with(dir))
            {
                return true;
            }
        } else if self.directories.iter().any(|dir| path.starts_with(dir)) {
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests;
