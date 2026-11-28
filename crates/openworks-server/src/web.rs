use std::path::Path;

use crate::http::Answer;

/// Answers a file of the web bundle under `root`, else its `index.html` for a screen's path.
pub fn file(root: &Path, path: &str) -> Answer {
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if parts.iter().any(|part| part.starts_with('.') || part.contains('\\')) {
        return Answer::text(404, "no such file");
    }
    let wanted = parts.iter().fold(root.to_path_buf(), |path, part| path.join(part));
    let found = if wanted.is_file() {
        wanted
    } else {
        root.join("index.html")
    };
    match std::fs::read(&found) {
        Ok(body) => Answer {
            status: 200,
            kind: kind(&found),
            body,
        },
        Err(_) => Answer::text(404, "no such file"),
    }
}

fn kind(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json" | "map") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("woff2") => "font/woff2",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
