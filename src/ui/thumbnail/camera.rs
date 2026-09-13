// SPDX-License-Identifier: MIT

use std::io::Write;

use super::*;

const MAX_ICON_BYTES: usize = 1024 * 1024;
const ICON_TIMEOUT: Duration = Duration::from_secs(15);

pub(super) async fn render(path: &Path, cancellation: &Cancellation) -> Result<Vec<u8>, String> {
    if cancellation.is_cancelled() {
        return Err("Camera thumbnail cancelled".into());
    }
    let uri = path.to_str().ok_or("Invalid camera URI")?;
    let file = gio::File::for_uri(uri);
    let download = async {
        let info = file
            .query_info_future(
                "preview::icon",
                gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                glib::Priority::DEFAULT,
            )
            .await
            .map_err(|error| error.to_string())?;
        let icon = info
            .attribute_object("preview::icon")
            .and_then(|object| object.downcast::<gio::LoadableIcon>().ok())
            .ok_or("The camera did not provide a thumbnail")?;
        let (stream, _) = icon
            .load_future(256)
            .await
            .map_err(|error| error.to_string())?;
        let bytes = read_icon(&stream, MAX_ICON_BYTES).await?;
        stream
            .close_future(glib::Priority::DEFAULT)
            .await
            .map_err(|error| error.to_string())?;
        Ok::<_, String>(bytes)
    };
    let cancelled = async {
        loop {
            if cancellation.is_cancelled() {
                return Err("Camera thumbnail cancelled".to_owned());
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
    };
    let bytes = glib::future_with_timeout(
        ICON_TIMEOUT,
        futures_lite::future::race(download, cancelled),
    )
    .await
    .map_err(|_| "Camera thumbnail timed out".to_owned())??;
    let cancellation = cancellation.clone();
    // Camera preview icons are compressed, untrusted inputs too. Only the
    // sandbox's normalized PNG is handed to GTK, never the original icon bytes.
    gio::spawn_blocking(move || {
        if cancellation.is_cancelled() {
            return Err("Camera thumbnail cancelled".into());
        }
        let mut input = tempfile::Builder::new()
            .prefix("strata-camera-thumbnail-")
            .tempfile()
            .map_err(|error| error.to_string())?;
        input.write_all(&bytes).map_err(|error| error.to_string())?;
        render_thumbnail(input.path(), ThumbnailKind::Image, 256, &cancellation)
    })
    .await
    .map_err(|_| "Camera thumbnail worker failed".to_owned())?
}

async fn read_icon(stream: &impl IsA<gio::InputStream>, limit: usize) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    loop {
        let remaining = limit - output.len();
        let bytes = stream
            .read_bytes_future(8192.min(remaining + 1), glib::Priority::DEFAULT)
            .await
            .map_err(|error| error.to_string())?;
        if bytes.is_empty() {
            return Ok(output);
        }
        if bytes.len() > remaining {
            return Err("Camera thumbnail exceeds the size limit".into());
        }
        output.extend_from_slice(&bytes);
    }
}

#[cfg(test)]
mod tests;
