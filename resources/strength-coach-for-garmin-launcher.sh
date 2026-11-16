#!/bin/bash

exec 9>/run/lock/strength-coach-for-garmin-launcher.lock
flock -n 9 || exit 0

BIN_LOCATION=/usr/bin/strength-coach-for-garmin

USER_NAME=$(loginctl list-sessions --no-legend | awk '{print $3}' | head -n1)
if [[ -z "$USER_NAME" ]]; then
    exit 0
fi
echo "Username: $USER_NAME"

UID_NUM=$(id -u "$USER_NAME")

export XDG_RUNTIME_DIR="/run/user/$UID_NUM"

while IFS='=' read -r key value; do
    [[ -n "$key" ]] && export "$key=$value"
done < <(runuser -u "$USER_NAME" -- systemctl --user show-environment 2>/dev/null | grep -E '^(DISPLAY|WAYLAND_DISPLAY|XAUTHORITY|DBUS_SESSION_BUS_ADDRESS)=')

export IN_DEBUG=1

"$BIN_LOCATION" --poll
if [[ $? -eq 0 ]]; then
    setsid runuser -u "$USER_NAME" -- "$BIN_LOCATION" </dev/null >/dev/null 2>&1 &
fi

