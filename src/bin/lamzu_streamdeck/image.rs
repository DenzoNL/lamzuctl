//! SVG button images.

use base64::{Engine, engine::general_purpose::STANDARD};
use lamzuctl::BatteryStatus;

/// Render a 144x144 button with a big title and, if `battery` is given, a
/// battery-level bar filling from the bottom plus a percentage footer.
/// Without a battery reading, `status` is shown in the footer instead.
///
/// Returns a data URI suitable for Stream Deck's `setImage`.
pub fn battery_bar_image(title: &str, battery: Option<BatteryStatus>, status: &str) -> String {
    let (bar, footer) = match battery {
        Some(b) => {
            let pct = b.percentage.min(100);
            let (color, highlight) = if b.charging {
                ("#4FC3F7", "#81D4FA") // light blue
            } else if pct > 50 {
                ("#4CAF50", "#81C784") // green
            } else if pct > 20 {
                ("#FFC107", "#FFD54F") // amber
            } else {
                ("#F44336", "#E57373") // red
            };
            let height = u32::from(pct) * 144 / 100;
            let y = 144 - height;
            let bar = format!(
                r##"<defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{color}" stop-opacity="0.8"/><stop offset="1" stop-color="{color}" stop-opacity="0.3"/></linearGradient></defs>
  <rect x="0" y="{y}" width="144" height="{height}" fill="url(#g)"/>
  <rect x="0" y="{y}" width="144" height="3" fill="{highlight}" opacity="0.9"/>"##
            );
            let bolt = if b.charging { " \u{26A1}" } else { "" };
            (bar, format!("{pct}%{bolt}"))
        }
        None => (String::new(), escape_xml(status)),
    };

    let title = escape_xml(title);
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="144" height="144" viewBox="0 0 144 144">
  <rect width="144" height="144" fill="#1a1a1a"/>
  {bar}
  <text x="72" y="62" text-anchor="middle" dominant-baseline="middle" font-family="Arial, sans-serif" font-size="42" font-weight="bold" fill="#000" opacity="0.4">{title}</text>
  <text x="72" y="60" text-anchor="middle" dominant-baseline="middle" font-family="Arial, sans-serif" font-size="42" font-weight="bold" fill="#fff">{title}</text>
  <text x="72" y="108" text-anchor="middle" font-family="Arial, sans-serif" font-size="24" font-weight="bold" fill="#fff" opacity="0.95">{footer}</text>
</svg>"##
    );

    format!("data:image/svg+xml;base64,{}", STANDARD.encode(svg))
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(uri: &str) -> String {
        let b64 = uri.strip_prefix("data:image/svg+xml;base64,").unwrap();
        String::from_utf8(STANDARD.decode(b64).unwrap()).unwrap()
    }

    #[test]
    fn bar_and_footer() {
        let svg = decode(&battery_bar_image(
            "P1",
            Some(BatteryStatus {
                percentage: 50,
                charging: false,
            }),
            "",
        ));
        assert!(svg.contains(r#"y="72" width="144" height="72""#));
        assert!(svg.contains(">50%<"));
        assert!(svg.contains("#FFC107"));
    }

    #[test]
    fn status_without_battery_and_escaping() {
        let svg = decode(&battery_bar_image("<&>", None, "asleep"));
        assert!(svg.contains("&lt;&amp;&gt;"));
        assert!(svg.contains(">asleep<"));
        assert!(!svg.contains("linearGradient"));
    }
}
