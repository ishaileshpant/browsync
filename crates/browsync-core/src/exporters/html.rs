use std::collections::BTreeMap;

use crate::models::Bookmark;

/// Export bookmarks as Netscape Bookmark HTML format
/// This format is importable by all major browsers
pub fn export(bookmarks: &[Bookmark]) -> String {
    let mut html = String::from(
        "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n\
         <!-- This is an automatically generated file.\n\
              It will be read and overwritten.\n\
              DO NOT EDIT! -->\n\
         <META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n\
         <TITLE>Bookmarks</TITLE>\n\
         <H1>Bookmarks</H1>\n\
         <DL><p>\n",
    );

    // Build tree structure from flat folder paths
    let tree = build_tree(bookmarks);
    render_tree(&tree, &mut html, 1);

    html.push_str("</DL><p>\n");
    html
}

#[derive(Default)]
struct FolderNode {
    bookmarks: Vec<(String, String, i64)>, // (title, url, timestamp)
    children: BTreeMap<String, FolderNode>,
}

fn build_tree(bookmarks: &[Bookmark]) -> FolderNode {
    let mut root = FolderNode::default();

    for bm in bookmarks {
        let mut node = &mut root;
        for folder in &bm.folder_path {
            node = node.children.entry(folder.clone()).or_default();
        }
        node.bookmarks
            .push((bm.title.clone(), bm.url.clone(), bm.created_at.timestamp()));
    }

    root
}

fn render_tree(node: &FolderNode, html: &mut String, depth: usize) {
    let indent = "    ".repeat(depth);

    for (name, child) in &node.children {
        let escaped_name = html_escape(name);
        html.push_str(&format!("{indent}<DT><H3>{escaped_name}</H3>\n"));
        html.push_str(&format!("{indent}<DL><p>\n"));
        render_tree(child, html, depth + 1);
        html.push_str(&format!("{indent}</DL><p>\n"));
    }

    for (title, url, ts) in &node.bookmarks {
        let escaped_title = html_escape(title);
        let escaped_url = html_escape(url);
        html.push_str(&format!(
            "{indent}<DT><A HREF=\"{escaped_url}\" ADD_DATE=\"{ts}\">{escaped_title}</A>\n"
        ));
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn make_bm(url: &str, title: &str, path: Vec<&str>) -> Bookmark {
        Bookmark {
            id: Uuid::new_v4(),
            url: url.to_string(),
            title: title.to_string(),
            folder_path: path.into_iter().map(String::from).collect(),
            tags: vec![],
            favicon_url: None,
            source_browser: crate::models::Browser::Chrome,
            source_id: String::new(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            synced_at: Utc::now(),
        }
    }

    #[test]
    fn test_export_html_structure() {
        let bookmarks = vec![
            make_bm("https://example.com", "Example", vec!["Toolbar"]),
            make_bm("https://rust-lang.org", "Rust", vec!["Toolbar", "Dev"]),
            make_bm("https://python.org", "Python", vec!["Toolbar", "Dev"]),
        ];

        let html = export(&bookmarks);
        assert!(html.contains("NETSCAPE-Bookmark-file-1"));
        assert!(html.contains("<H3>Toolbar</H3>"));
        assert!(html.contains("<H3>Dev</H3>"));
        assert!(html.contains("https://example.com"));
        assert!(html.contains("https://rust-lang.org"));
    }

    #[test]
    fn test_html_escape() {
        let bm = make_bm("https://x.com?a=1&b=2", "Test <b>bold</b>", vec![]);
        let html = export(&[bm]);
        assert!(html.contains("&amp;"));
        assert!(html.contains("&lt;b&gt;"));
    }
}
