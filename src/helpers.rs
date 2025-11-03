use std::collections::HashSet;

use chrono::{DateTime, Utc};
use gtk::glib;
use glib::prelude::Cast;
use gtk4 as gtk;
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::categories::icon_resource_for_package;
use crate::details::{DiscoverDetail, InstalledDetail};
use crate::types::PackageInfo;
use crate::xbps::{
    format_download_size, format_size, query_package_metadata, query_pkgsize_bytes,
    query_repo_package_info, run_xbps_query_dependencies, run_xbps_query_required_by,
};

pub(crate) fn clear_listbox(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

pub(crate) fn set_toggle_button_state(button: &gtk::ToggleButton, active: bool) {
    if button.is_active() != active {
        button.set_active(active);
    }
}

pub(crate) fn populate_spotlight_list(list: &gtk::ListBox, packages: &[PackageInfo]) {
    clear_listbox(list);
    for pkg in packages {
        let row = build_spotlight_row(pkg);
        list.append(&row);
    }
}

pub(crate) fn select_row_if_attached(list: &gtk::ListBox, row: &gtk::ListBoxRow) {
    if row
        .parent()
        .and_then(|widget| widget.downcast::<gtk::ListBox>().ok())
        .map(|parent| parent.as_ptr() == list.as_ptr())
        .unwrap_or(false)
    {
        list.select_row(Some(row));
    }
}

pub(crate) fn set_download_label(
    label: &gtk::Label,
    detail_bytes: Option<u64>,
    detail_text: Option<&str>,
    pkg_bytes: Option<u64>,
    pkg_text: Option<&str>,
) {
    if let Some(text) = detail_text {
        label.set_text(text);
    } else if let Some(bytes) = detail_bytes.or(pkg_bytes) {
        label.set_text(&format_size(bytes));
    } else if let Some(text) = pkg_text {
        label.set_text(text);
    } else {
        label.set_text("—");
    }
}

pub(crate) fn format_relative_time(timestamp: DateTime<Utc>) -> String {
    let now = Utc::now();
    let delta = now.signed_duration_since(timestamp);

    if delta.num_seconds() <= 0 {
        return "just now".to_string();
    }

    if delta.num_minutes() < 1 {
        return "just now".to_string();
    }

    if delta.num_hours() < 1 {
        let minutes = delta.num_minutes();
        return format!(
            "{} minute{} ago",
            minutes,
            if minutes == 1 { "" } else { "s" }
        );
    }

    if delta.num_hours() < 24 {
        let hours = delta.num_hours();
        return format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" });
    }

    let days = delta.num_days();
    if days < 7 {
        return format!("{} day{} ago", days, if days == 1 { "" } else { "s" });
    }

    let weeks = days / 7;
    if weeks < 5 {
        return format!("{} week{} ago", weeks, if weeks == 1 { "" } else { "s" });
    }

    let months = days / 30;
    if months < 12 {
        return format!("{} month{} ago", months, if months == 1 { "" } else { "s" });
    }

    let years = days / 365;
    if years < 1 {
        return "about a year ago".to_string();
    }

    format!("{} year{} ago", years, if years == 1 { "" } else { "s" })
}

pub(crate) fn glib_datetime_to_chrono(dt: &glib::DateTime) -> Option<DateTime<Utc>> {
    let utc = dt.to_timezone(&glib::TimeZone::utc()).ok()?;
    let seconds = utc.to_unix();
    let micros = utc.microsecond() as i64;
    DateTime::<Utc>::from_timestamp_micros(seconds * 1_000_000 + micros)
}

pub(crate) fn sanitize_contact_field(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(start) = trimmed.find('<') {
        if let Some(end_rel) = trimmed[start + 1..].find('>') {
            let end = start + 1 + end_rel;
            let name = trimmed[..start].trim();
            let email = trimmed[start + 1..end].trim();

            return match (name.is_empty(), email.is_empty()) {
                (false, false) => format!("{} ({})", name, email),
                (false, true) => name.to_string(),
                (true, false) => email.to_string(),
                (true, true) => String::new(),
            };
        }
    }

    trimmed.replace('<', "").replace('>', "").trim().to_string()
}

pub(crate) fn set_link_label(label: &gtk::Label, url: Option<&str>) {
    if let Some(url) = url {
        let display = glib::markup_escape_text(url);
        let href = glib::markup_escape_text(url);
        label.set_markup(&format!("<a href=\"{href}\">{display}</a>"));
        label.set_visible(true);
        label.set_tooltip_text(Some(url));
    } else {
        label.set_text("");
        label.set_visible(false);
        label.set_tooltip_text(None);
    }
}

pub(crate) fn package_matches_filter(pkg: &PackageInfo, filter_lower: &str) -> bool {
    let needle = filter_lower.trim();
    if needle.is_empty() {
        return true;
    }

    pkg.name_lower.contains(needle)
        || pkg.version_lower.contains(needle)
        || pkg.description_lower.contains(needle)
}

pub(crate) fn query_installed_detail(
    package: &str,
    _installed_set: &HashSet<String>,
) -> Result<InstalledDetail, String> {
    let mut detail = InstalledDetail::default();

    match query_pkgsize_bytes(package) {
        Ok(Some(bytes)) => {
            detail.download_bytes = Some(bytes);
            detail.download_formatted = Some(format_download_size(bytes));
        }
        Ok(None) => {}
        Err(err) => detail.download_error = Some(err),
    }

    match run_xbps_query_required_by(package) {
        Ok(required) => {
            detail.required_by = required;
        }
        Err(err) => detail.required_by_error = Some(err),
    }

    let metadata = query_package_metadata(package);
    detail.long_description = metadata.long_desc;
    detail.homepage = metadata.homepage;
    detail.maintainer = metadata.maintainer;
    detail.license = metadata.license;

    Ok(detail)
}

pub(crate) fn query_discover_detail(package: &str) -> Result<DiscoverDetail, String> {
    let info = query_repo_package_info(package)?;
    let dependency_names = run_xbps_query_dependencies(package)
        .unwrap_or_default()
        .into_iter()
        .map(|dep| dep.name)
        .collect::<Vec<_>>();

    let mut detail = DiscoverDetail::with_dependencies(&info, dependency_names);
    detail.download_bytes = info.download_bytes;
    detail.download = info
        .download_size
        .clone()
        .or_else(|| info.download_bytes.map(format_size));

    let metadata = query_package_metadata(package);
    if let Some(long_desc) = metadata.long_desc {
        detail.description = Some(long_desc);
    }
    detail.homepage = metadata.homepage;
    detail.maintainer = metadata.maintainer;
    detail.license = metadata.license;
    detail.repository = metadata.repository.or(info.repository.clone());

    Ok(detail)
}

pub(crate) fn detail_download_bytes(package: &str) -> Option<u64> {
    query_pkgsize_bytes(package).ok().flatten()
}

fn build_spotlight_row(pkg: &PackageInfo) -> adw::ActionRow {
    let subtitle = if pkg.description.is_empty() {
        pkg.version.clone()
    } else if pkg.version.is_empty() {
        pkg.description.clone()
    } else {
        format!("{} - {}", pkg.version, pkg.description)
    };

    let row = adw::ActionRow::builder()
        .title(pkg.name.as_str())
        .subtitle(subtitle.as_str())
        .build();
    row.set_title_lines(1);
    row.set_subtitle_lines(2);
    row.set_activatable(true);
    row.set_focusable(true);
    row.set_tooltip_text(Some("Open details for this package."));

    let icon = gtk::Image::from_resource(icon_resource_for_package(&pkg.name));
    icon.set_pixel_size(28);
    icon.set_margin_end(12);
    icon.set_valign(gtk::Align::Center);
    row.add_prefix(&icon);

    let suffix_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();

    if pkg.installed {
        let badge = gtk::Label::new(Some("Installed"));
        badge.add_css_class("tag");
        suffix_box.append(&badge);
    }

    if let Some(build_date) = pkg.build_date {
        let relative = format_relative_time(build_date);
        let time_label = gtk::Label::builder()
            .label(relative.as_str())
            .halign(gtk::Align::End)
            .build();
        time_label.add_css_class("dim-label");
        time_label.set_justify(gtk::Justification::Right);
        time_label.set_xalign(1.0);
        suffix_box.append(&time_label);
    }

    if suffix_box.first_child().is_some() {
        row.add_suffix(&suffix_box);
    }

    row
}
