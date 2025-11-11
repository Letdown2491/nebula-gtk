# Changelog

## 1.3.5
- Added Recent Activity to hamburger menu to track install/remove/update operations.
- Added activity indicators to packages to easily distinguish success/error states in install/remove/update operations.
- Added cached package cleanup hooked to 'xbps-remove -o'.
- Added optional spinner to allow users to select how many cached package versions to keep (max 5).
- Added Waypoint module to integrate Btrfs snapshot creation on system upgrade.
- Added preferences UI + message handler for Waypoint integration.
- Added dedicated start/end window controls plus mirrored header logos so the system layout moves the branding to the free side.
- Added toast notification for Hold/Unhold package actions.
- Updated empty search state pages to be more informative.
- Wired notify::empty listeners to toggle which logo is shown as GNOME switches button placement, keeping the menu button available either way.
- Moved Tools page notifications to application footer for cleaner UI and save vertical space.
- Reordered menu items.
- Fixed a critical error that allowed users to remove multiple packages without confirmation even when the confirmation setting was enabled in Preferences.

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
