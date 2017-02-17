#! /bin/bash

set -e

/bin/systemctl daemon-reload

# Restart evebox if running.
if /bin/systemctl status evebox > /dev/null; then
    echo "Restarting evebox."
    /bin/systemctl restart evebox
fi

# Restart evebox-agent if running.
if /bin/systemctl status evebox-agent > /dev/null; then
    echo "Restarting evebox-agent."
    /bin/systemctl restart evebox-agent
fi
