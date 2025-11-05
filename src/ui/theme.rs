use std::f64::consts::PI;

use gtk4 as gtk;
use gtk4::cairo;
use gtk4::prelude::{DrawingAreaExtManual, WidgetExt};
use libadwaita as adw;

#[derive(Clone, Copy)]
pub(crate) enum ThemeGlyph {
    System,
    Light,
    Dark,
}

pub(crate) fn apply_theme_css_class(window: &adw::ApplicationWindow, is_dark: bool) {
    window.remove_css_class("nebula-window-light");
    window.remove_css_class("nebula-window-dark");
    if is_dark {
        window.add_css_class("nebula-window-dark");
    } else {
        window.add_css_class("nebula-window-light");
    }
}

pub(crate) fn build_theme_icon(mode: ThemeGlyph) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .content_width(24)
        .content_height(24)
        .build();
    area.set_draw_func(move |_area, cr, width, height| {
        cr.set_antialias(cairo::Antialias::Best);
        let size = f64::from(width.min(height));
        let radius = (size / 2.0) - 2.0;
        let cx = f64::from(width) / 2.0;
        let cy = f64::from(height) / 2.0;

        let _ = cr.save();
        cr.arc(cx, cy, radius, 0.0, 2.0 * PI);
        cr.clip();

        match mode {
            ThemeGlyph::System => {
                cr.set_source_rgb(1.0, 1.0, 1.0);
                cr.rectangle(cx, cy - radius, radius, radius * 2.0);
                let _ = cr.fill();

                cr.set_source_rgb(0.1, 0.1, 0.1);
                cr.rectangle(cx - radius, cy - radius, radius, radius * 2.0);
                let _ = cr.fill();
            }
            ThemeGlyph::Light => {
                cr.set_source_rgb(1.0, 1.0, 1.0);
                let _ = cr.paint();
            }
            ThemeGlyph::Dark => {
                cr.set_source_rgb(0.1, 0.1, 0.1);
                let _ = cr.paint();
            }
        }

        let _ = cr.restore();
        cr.set_line_width(2.0);
        cr.set_source_rgb(0.2, 0.2, 0.2);
        cr.arc(cx, cy, radius, 0.0, 2.0 * PI);
        let _ = cr.stroke();
    });

    area
}
