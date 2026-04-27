/// Minimal HTML escaper — covers the five chars that matter for HTML
/// attribute and text contexts. Keeps the crate dep-free of an escape lib.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

pub fn page(title: &str, body: &str) -> String {
    format!(
        r##"<!doctype html>
<html lang="en"><head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title} — witness-viz</title>
<link rel="stylesheet" href="/assets/styles.css">
<script src="/assets/htmx.min.js" defer></script>
</head><body>
<div class="shell">
  <nav class="sidebar">
    <div class="brand"><a href="/">witness-viz</a></div>
    <ul>
      <li><a href="/">Overview</a></li>
      <li><a href="/api/v1/summary">JSON summary</a></li>
    </ul>
    <div class="footer">v{version}</div>
  </nav>
  <main class="content">
    {body}
  </main>
</div>
</body></html>"##,
        title = escape(title),
        body = body,
        version = env!("CARGO_PKG_VERSION"),
    )
}
