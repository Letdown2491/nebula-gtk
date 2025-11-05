# Changelog

## 1.2.3 (Unreleased)
- Added dedicated start/end window controls plus mirrored header logos so the system layout moves the branding to the free side.
- Wired notify::empty listeners to toggle which logo is shown as GNOME switches button placement, keeping the menu button available either way.

## 1.2.2
- Added hold/unhold feature for installed packages to prevent updates on specific package versions.
- When the app details pane is opened in the Installed page, hide the buttons from the package list, and show then again when app details pane is closed.
- Fixed a bug that kept app process running after quitting.
- Updated version number.
- Updated CHANGELOD.

## 1.1.4
- Added state changes to Updates page to visually track status by button text.
- Added log viewer to track full update state from XBPS.
- After an update is completed, refresh the update list from cache instead of rerunning xbps-install -Sun.
- Fixed theme switcher icons.
- Updated About Nebula dialog styling.
- Minor bugfixes.
- Updated version number.

## 1.0.0
- Discover catalog with curated categories, recent updates, and full-text search for Void Linux software.
- Split-view app details pane with homepage links, dependency navigation, install/remove actions, and consistent layout across sections.
- Installed and Updates dashboards with queued operations, update checks after startup, and XBPS-powered upgrade workflows.
- Startup experience tuned for responsiveness, including deferred update refresh and centered toolbar navigation.
- Bundled icons, desktop entry, and category data generation tooling for distribution builds.
