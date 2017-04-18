#! /usr/bin/env bash

# Kill background jobs on exit.
trap 'kill $(jobs -p)' EXIT

# Do an initial make so the output directory exists.
make html

# Start reflex and save its pid.
reflex -- make html &

# Now start devd.
devd -l _build/html
