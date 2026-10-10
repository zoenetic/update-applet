updates = Updates
checking = Checking for updates…
up-to-date = Your system is up to date
updates-available = { $count ->
    [one] 1 update available
   *[other] { $count } updates available
}
security-updates = { $count ->
    [one] includes 1 security update
   *[other] includes { $count } security updates
}
last-checked = Last checked { $minutes ->
    [0] just now
    [one] 1 minute ago
   *[other] { $minutes } minutes ago
}
last-checked-hours = Last checked { $hours ->
    [one] 1 hour ago
   *[other] { $hours } hours ago
}
system-packages = System packages
flatpak-apps = Flatpak apps
no-packagekit = PackageKit is not installed, system updates are unavailable
check-now = Check for updates
install-now = Install all updates
install-on-restart = Install on next restart
installing-system = Installing system updates…
installing-flatpak = Updating Flatpak apps…
downloading-system = Downloading system updates…
install-done = Updates installed
offline-prepared = Updates will be installed on the next restart
restart-now = Restart now…
error = Something went wrong: { $details }
include-flatpak = Include Flatpak apps
show-count = Show count on panel
check-interval = Check automatically
check-every = { $hours ->
    [0] Never
    [one] Every hour
   *[other] Every { $hours } hours
}
open-store = Open COSMIC Store…
