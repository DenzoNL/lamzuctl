//! Image Generation for Stream Deck
//!
//! Generates SVG images for button display with battery indicator.

use base64::{engine::general_purpose::STANDARD, Engine};

/// Generate an SVG image with a battery bar background and text overlay
/// Returns a data URI suitable for Stream Deck's SetImage command
pub fn generate_battery_bar_image(title: &str, battery_percentage: u8, charging: bool) -> String {
    let percentage = battery_percentage.min(100) as f32;

    // Colors based on battery level
    let (bar_color, bar_color_light) = if charging {
        ("#4FC3F7", "#81D4FA") // Light blue when charging
    } else if percentage > 50.0 {
        ("#4CAF50", "#81C784") // Green
    } else if percentage > 20.0 {
        ("#FFC107", "#FFD54F") // Yellow/amber
    } else {
        ("#F44336", "#E57373") // Red
    };

    // Calculate bar height (fills from bottom)
    let bar_height = (percentage / 100.0 * 144.0) as i32;
    let bar_y = 144 - bar_height;

    // Charging indicator - lightning bolt or plug symbol
    let charging_indicator = if charging { " \u{26A1}" } else { "" }; // ⚡

    let escaped_title = escape_xml(title);

    // SVG with gradient bar, top highlight, and centered text
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="144" height="144" viewBox="0 0 144 144">
  <defs>
    <!-- Gradient for battery bar - fades toward bottom -->
    <linearGradient id="barGrad" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" style="stop-color:{bar_color};stop-opacity:0.8"/>
      <stop offset="100%" style="stop-color:{bar_color};stop-opacity:0.3"/>
    </linearGradient>
  </defs>

  <!-- Background -->
  <rect width="144" height="144" fill="#1a1a1a"/>

  <!-- Battery bar with gradient -->
  <rect x="0" y="{bar_y}" width="144" height="{bar_height}" fill="url(#barGrad)"/>

  <!-- Top border/highlight on the bar -->
  <rect x="0" y="{bar_y}" width="144" height="3" fill="{bar_color_light}" opacity="0.9"/>

  <!-- Main title text shadow -->
  <text x="72" y="62" text-anchor="middle" dominant-baseline="middle" font-family="Arial, sans-serif" font-size="42" font-weight="bold" fill="#000" opacity="0.4">{escaped_title}</text>

  <!-- Main title text -->
  <text x="72" y="60" text-anchor="middle" dominant-baseline="middle" font-family="Arial, sans-serif" font-size="42" font-weight="bold" fill="#fff">{escaped_title}</text>

  <!-- Battery percentage with charging indicator -->
  <text x="72" y="108" text-anchor="middle" font-family="Arial, sans-serif" font-size="24" font-weight="bold" fill="#fff" opacity="0.95">{percentage:.0}%{charging_indicator}</text>
</svg>"##,
        bar_y = bar_y,
        bar_height = bar_height,
        bar_color = bar_color,
        bar_color_light = bar_color_light,
        escaped_title = escaped_title,
        percentage = percentage,
        charging_indicator = charging_indicator
    );

    // Convert to base64 data URI
    let base64_svg = STANDARD.encode(svg.as_bytes());
    format!("data:image/svg+xml;base64,{}", base64_svg)
}

/// Generate a simple text-only image (no battery bar)
#[allow(dead_code)]
pub fn generate_text_image(title: &str) -> String {
    let escaped_title = escape_xml(title);

    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="144" height="144" viewBox="0 0 144 144">
  <rect width="144" height="144" fill="#1a1a1a"/>
  <text x="72" y="72" text-anchor="middle" dominant-baseline="middle" font-family="Arial, sans-serif" font-size="36" font-weight="bold" fill="#fff">{escaped_title}</text>
</svg>"##,
        escaped_title = escaped_title
    );

    let base64_svg = STANDARD.encode(svg.as_bytes());
    format!("data:image/svg+xml;base64,{}", base64_svg)
}

/// Escape special XML characters
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
