#! /bin/bash

set -e

# Restart evebox if running.
if systemctl status evebox > /dev/null; then
    systemctl restart evebox
fi
