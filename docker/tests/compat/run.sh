#!/usr/bin/env bash
#
# SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
# SPDX-License-Identifier: MIT
#
# Run the built-in `evebox test elastic` compatibility test against a matrix
# of Elasticsearch and OpenSearch container versions.
#
# For each version this script:
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
# Edit freely — image tags must exist in the respective registries. The matrix
# can also be overridden on the command line, e.g.:
#   ./run.sh os|2.6.0 es|7.10.2
VERSIONS=(
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

if [ -z "$CONTAINER" ]; then
    echo "error: no container runtime found (install podman or docker)" >&2
    exit 1
fi
if ! command -v curl >/dev/null 2>&1; then
    echo "error: curl is required" >&2
    exit 1
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

echo "Runtime:  $CONTAINER"
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

for entry in "${VERSIONS[@]}"; do
    engine="${entry%%|*}"
    version="${entry##*|}"

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

    # Pull first (with retries) so transient registry failures are distinct
    # from container start failures.
    if ! pull_image "$image"; then
        echo "    failed to pull $image after retries"
        results+=("$label|PULL-FAIL|")
        continue
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
    out="$("$EVEBOX" test elastic -e "http://localhost:$PORT" \
        --limit "$LIMIT" --json "$EVE_DIR" 2>/tmp/evebox-compat.log)"
    rc=$?
    echo "$out"

    summary="$(printf '%s' "$out" | grep -oE '"(passed|failed|known|skipped)": *[0-9]+' | tr '\n' ' ')"
    if [ "$rc" = "0" ]; then
        results+=("$label|OK|$summary")
    else
        results+=("$label|FAIL($rc)|$summary")
        echo "--- evebox stderr ---"
        tail -n 20 /tmp/evebox-compat.log
    fi

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
