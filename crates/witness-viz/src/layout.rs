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

/// Layout options for the page chrome.
///
/// Two callers today:
/// - [`PageOpts::serve`] — axum handlers, links resolve via the live
///   router, HTMX is included for partial swaps, JSON-summary link is shown.
/// - [`PageOpts::static_export`] — the export driver, links are
///   relative to the page's depth in the output tree, HTMX is omitted
///   (no axum to swap against), JSON-summary link is hidden (no API
///   in a static dump).
pub struct PageOpts<'a> {
    /// Where the CSS lives, relative to the page being rendered.
    /// Serve mode: `"/assets/"`. Static export: e.g. `"../_assets/"`.
    pub asset_prefix: &'a str,
    /// Href for the brand and "Overview" sidebar link.
    pub overview_href: &'a str,
    /// Emit the HTMX `<script>` tag. False for static export.
    pub include_htmx: bool,
    /// Show the "JSON summary" sidebar link. False for static export.
    pub include_api_link: bool,
}

impl PageOpts<'static> {
    /// Defaults for serve mode — matches the byte shape produced
    /// before the static-export refactor.
    pub const fn serve() -> Self {
        Self {
            asset_prefix: "/assets/",
            overview_href: "/",
            include_htmx: true,
            include_api_link: true,
        }
    }
}

/// Render the page chrome around `body`. The `opts` carries the
/// asset/link knobs that differ between serve and static-export modes.
pub fn page_with(title: &str, body: &str, opts: &PageOpts<'_>) -> String {
    let mut head_extra = String::new();
    if opts.include_htmx {
        head_extra.push_str(&format!(
            r#"<script src="{}htmx.min.js" defer></script>"#,
            opts.asset_prefix
        ));
    }

    let mut nav_extra = String::new();
    if opts.include_api_link {
        nav_extra.push_str(r#"      <li><a href="/api/v1/summary">JSON summary</a></li>"#);
        nav_extra.push('\n');
    }

    format!(
        r##"<!doctype html>
<html lang="en"><head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title} — witness-viz</title>
<link rel="stylesheet" href="{asset_prefix}styles.css">
{head_extra}
</head><body>
<div class="shell">
  <nav class="sidebar">
    <div class="brand"><a href="{overview}">witness-viz</a></div>
    <ul>
      <li><a href="{overview}">Overview</a></li>
{nav_extra}    </ul>
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
        asset_prefix = opts.asset_prefix,
        overview = opts.overview_href,
        head_extra = head_extra,
        nav_extra = nav_extra,
    )
}

/// Serve-mode convenience: equivalent to `page_with(title, body,
/// &PageOpts::serve())`. Kept short because every axum handler calls it.
pub fn page(title: &str, body: &str) -> String {
    page_with(title, body, &PageOpts::serve())
}
