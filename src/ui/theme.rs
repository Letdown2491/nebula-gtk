use std::f64::consts::PI;

use gtk4 as gtk;
use gtk4::prelude::{DrawingAreaExt, DrawingAreaExtManual, WidgetExt};
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
    let area = gtk::DrawingArea::new();
    area.set_content_width(16);
    area.set_content_height(16);
    area.set_draw_func(move |_area, cr, width, height| {
        cr.set_line_width(2.0);
        cr.set_source_rgb(0.2, 0.2, 0.2);
        cr.arc(
            width as f64 / 2.0,
            height as f64 / 2.0,
            (width as f64 / 2.0) - 2.0,
            0.0,
            2.0 * PI,
        );
        let _ = cr.stroke();

        match mode {
            ThemeGlyph::System => {
                cr.set_source_rgb(0.2, 0.2, 0.2);
                cr.arc(
                    width as f64 / 2.0,
                    height as f64 / 2.0,
                    (width as f64 / 2.0) - 4.0,
                    0.0,
                    2.0 * PI,
                );
                let _ = cr.fill();
            }
            ThemeGlyph::Light => {
                cr.set_source_rgb(1.0, 1.0, 1.0);
                cr.arc(
                    width as f64 / 2.0,
                    height as f64 / 2.0,
                    (width as f64 / 2.0) - 4.0,
                    0.0,
                    2.0 * PI,
                );
                let _ = cr.fill();
            }
            ThemeGlyph::Dark => {
                cr.set_source_rgb(0.1, 0.1, 0.1);
                cr.arc(
                    width as f64 / 2.0,
                    height as f64 / 2.0,
                    (width as f64 / 2.0) - 4.0,
                    0.0,
                    2.0 * PI,
                );
                let _ = cr.fill();
            }
        }
    });

    area
}
