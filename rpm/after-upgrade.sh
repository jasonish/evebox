#! /bin/bash

set -e

/bin/systemctl daemon-reload

# Restart evebox if running.
if /bin/systemctl status evebox > /dev/null; then
    /bin/systemctl restart evebox
fi
