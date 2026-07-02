#!/usr/bin/env bash
#
# SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
# SPDX-License-Identifier: MIT
#
# One-shot datastore integration test: run the built-in `evebox test`
# compatibility tests against SQLite and a matrix of Elasticsearch and
# OpenSearch container versions.
#
# The "sqlite" matrix entry runs `evebox test sqlite` directly (no container).
# For each es/os version this script:
#   1. starts a single-node, security-disabled container on :9200,
#   2. waits for it to become healthy,
#   3. runs `evebox test elastic` against it,
#   4. records the result and stops the container.
#
# Containers are run one at a time (they all bind :9200).
#
# Environment overrides:
#   CONTAINER  container runtime to use (default: podman if present, else docker)
#   EVE_DIR    directory of EVE json files to sample (default: <repo>/../eve)
#   LIMIT      max events to import per run (default: 20000)
#   EVEBOX     path to the evebox binary (default: build & use target/release)
#   PORT       host port to bind (default: 9200)
#   KEEP_GOING set to 1 to continue after a container fails to start

set -u

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

CONTAINER="${CONTAINER:-$(command -v podman || command -v docker || true)}"
EVE_DIR="${EVE_DIR:-$REPO_ROOT/../eve}"
LIMIT="${LIMIT:-20000}"
PORT="${PORT:-9200}"
NAME="evebox-compat"

# Fully-qualified image names so podman (which has no default registry) works.
ES_IMAGE="docker.elastic.co/elasticsearch/elasticsearch"
OS_IMAGE="docker.io/opensearchproject/opensearch"

# The version matrix. Each entry is "engine|version"; engine is "es" or "os".
# The bare entry "sqlite" tests the built-in SQLite datastore (no container
# needed). Edit freely — image tags must exist in the respective registries.
# The matrix can also be overridden on the command line, e.g.:
#   ./run.sh sqlite os|2.6.0 es|7.10.2
VERSIONS=(
    "sqlite"
    "es|7.10.2"
    "es|7.17.28"
    "es|8.19.0"
    "es|9.4.2"
    "os|2.6.0"
    "os|2.19.5"
    "os|3.6.0"
    "os|3.7.0"
)
# NOTE: avoid Elasticsearch 7.17.0–7.17.x-with-old-JDK; their bundled JDK 17.0.1
# crashes on cgroup v2 hosts (CgroupV2Subsystem NullPointerException) before ES
# starts. 7.17.28 (latest at time of writing: 7.17.30) bundles a fixed JDK.
if [ "$#" -gt 0 ]; then
    VERSIONS=("$@")
fi

# A container runtime is only needed for the es/os entries; a sqlite-only run
# (./run.sh sqlite) has no container dependency.
needs_container=0
for entry in "${VERSIONS[@]}"; do
    [ "$entry" = "sqlite" ] || needs_container=1
done

if [ "$needs_container" = "1" ]; then
    if [ -z "$CONTAINER" ]; then
        echo "error: no container runtime found (install podman or docker)" >&2
        exit 1
    fi
    if ! command -v curl >/dev/null 2>&1; then
        echo "error: curl is required" >&2
        exit 1
    fi
fi
if [ ! -d "$EVE_DIR" ]; then
    echo "error: EVE_DIR does not exist: $EVE_DIR" >&2
    echo "       set EVE_DIR to a directory of EVE json files" >&2
    exit 1
fi

# Build evebox unless a binary was provided.
if [ -z "${EVEBOX:-}" ]; then
    echo "Building evebox..."
    ( cd "$REPO_ROOT" && cargo build ) || exit 1
    EVEBOX="$REPO_ROOT/target/debug/evebox"
fi

echo "Runtime:  ${CONTAINER:-(none)}"
echo "EveBox:   $EVEBOX"
echo "EVE dir:  $EVE_DIR"
echo "Limit:    $LIMIT events"
echo

stop_container() {
    "$CONTAINER" rm -f "$NAME" >/dev/null 2>&1 || true
}
trap stop_container EXIT

# Pull an image, retrying a few times — Docker Hub pulls intermittently fail
# with auth/rate-limit errors ("unable to retrieve auth token").
pull_image() {
    img="$1"
    for attempt in 1 2 3; do
        if "$CONTAINER" pull "$img"; then
            return 0
        fi
        echo "    pull attempt $attempt for $img failed; retrying in 5s..."
        sleep 5
    done
    return 1
}

results=()

# Run an `evebox test ...` command and record a summary line for it under $1
# (the label). The JSON report is saved to a file, not echoed; on failure it
# is printed along with the evebox stderr tail.
run_and_record() {
    label="$1"
    shift
    report="/tmp/evebox-compat-$(printf '%s' "$label" | tr ' ' '-').json"
    out="$("$@" 2>/tmp/evebox-compat.log)"
    rc=$?
    printf '%s\n' "$out" >"$report"
    summary="$(printf '%s' "$out" |
        grep -oE '"(passed|failed|known|skipped)": *[0-9]+' |
        sed 's/"//g; s/: */=/' | tr '\n' ' ')"
    if [ "$rc" = "0" ]; then
        echo "    OK  ${summary}(report: $report)"
        results+=("$label|OK|$summary")
    else
        echo "    FAIL($rc)  $summary"
        echo "$out"
        echo "--- evebox stderr ---"
        tail -n 20 /tmp/evebox-compat.log
        results+=("$label|FAIL($rc)|$summary")
    fi
}

for entry in "${VERSIONS[@]}"; do
    engine="${entry%%|*}"
    version="${entry##*|}"

    if [ "$engine" = "sqlite" ]; then
        echo "=============================================================="
        echo ">>> sqlite"
        echo "=============================================================="
        run_and_record "sqlite" \
            "$EVEBOX" test sqlite --limit "$LIMIT" --json "$EVE_DIR"
        echo
        continue
    fi

    if [ "$engine" = "es" ]; then
        image="$ES_IMAGE:$version"
        env_args=(-e "discovery.type=single-node"
                  -e "xpack.security.enabled=false"
                  -e "ES_JAVA_OPTS=-Xms1g -Xmx1g")
        label="elasticsearch $version"
    else
        image="$OS_IMAGE:$version"
        env_args=(-e "discovery.type=single-node"
                  -e "DISABLE_SECURITY_PLUGIN=true"
                  -e "DISABLE_INSTALL_DEMO_CONFIG=true"
                  -e "OPENSEARCH_JAVA_OPTS=-Xms1g -Xmx1g")
        label="opensearch $version"
    fi

    echo "=============================================================="
    echo ">>> $label"
    echo "=============================================================="

    stop_container

    # Pull only if the image is not already local — these tags are effectively
    # immutable, so a local copy never needs refreshing. When a pull is needed
    # it is done first (with retries) so transient registry failures are
    # distinct from container start failures.
    if ! "$CONTAINER" image inspect "$image" >/dev/null 2>&1; then
        if ! pull_image "$image"; then
            echo "    failed to pull $image after retries"
            results+=("$label|PULL-FAIL|")
            continue
        fi
    fi

    # Not --rm: keep the container around on failure so we can read its logs.
    # --log-driver k8s-file: some hosts default to the 'none' driver, which
    # makes `logs` return nothing.
    if ! "$CONTAINER" run -d --name "$NAME" --log-driver k8s-file \
        -p "$PORT:9200" "${env_args[@]}" "$image" >/dev/null; then
        echo "    failed to start container"
        results+=("$label|START-FAIL|")
        continue
    fi

    # Wait for the node to answer on GET /.
    healthy=0
    for _ in $(seq 1 180); do
        code="$(curl -s -o /dev/null -w '%{http_code}' "http://localhost:$PORT/" 2>/dev/null || true)"
        if [ "$code" = "200" ]; then
            healthy=1
            break
        fi
        sleep 1
    done

    if [ "$healthy" != "1" ]; then
        echo "    container never became healthy; last log lines:"
        "$CONTAINER" logs "$NAME" 2>&1 | tail -n 15 | sed 's/^/      /'
        results+=("$label|UNHEALTHY|")
        stop_container
        continue
    fi

    # Run the compatibility test.
    run_and_record "$label" \
        "$EVEBOX" test elastic -e "http://localhost:$PORT" \
        --limit "$LIMIT" --json "$EVE_DIR"

    stop_container
    echo
done

echo "=============================================================="
echo "Summary"
echo "=============================================================="
for r in "${results[@]}"; do
    label="${r%%|*}"
    rest="${r#*|}"
    status="${rest%%|*}"
    detail="${rest#*|}"
    printf '  %-26s %-12s %s\n' "$label" "$status" "$detail"
done

# Non-zero exit if any run was not OK.
for r in "${results[@]}"; do
    status="$(printf '%s' "$r" | cut -d'|' -f2)"
    [ "$status" = "OK" ] || exit 1
done
exit 0
