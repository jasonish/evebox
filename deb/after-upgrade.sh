#! /bin/bash

set -e

if ! /bin/systemctl daemon-reload > /dev/null 2>&1; then
    # Exit now if this failed. May be running in a container.
    exit 0
fi

# Restart evebox if running.
if /bin/systemctl status evebox > /dev/null; then
    echo "Restarting evebox."
    /bin/systemctl restart evebox
fi
