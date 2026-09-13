# Changelog

Newest first. Each section is the release's notes, published verbatim.

## 0.1.0

- Search across pacman, the AUR, Flatpak and Snap from one box, with the same application from several sources shown as one row.
- Install, remove and update through one flow, with one password prompt per batch. On Arch every install or update runs the full system upgrade with the package included, because Arch supports nothing less.
- An Updates page that checks every source, lets you update everything or a selection, and says when a selection would be a partial upgrade on Arch.
- Application icons, descriptions and screenshots from AppStream and Flathub.
- The AUR search passes over words the AUR refuses as too common and says so when every word is.
- Flathub and the Snap Store are searched before Flatpak or snapd is installed. Installing from them sets the tool up first, and Settings has a button to set either up on its own.
- The Flatpak installation and AUR helper settings now take effect.
- The confirm step says how many times you may be asked for your password, and marks the steps that ask.
- Drivers through chwd on CachyOS and firmware through fwupd.
- BAP Store checks for its own new versions and installs them the way this copy was installed.
