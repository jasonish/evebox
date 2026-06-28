# Datastore compatibility tests

These tools verify that EveBox works against the Elasticsearch and OpenSearch
versions it claims to support (Elasticsearch >= 7.10, OpenSearch >= 2.6.0).

There are two pieces:

1. **`evebox test elastic`** — a built-in subcommand that loads a sample of
   real EVE events into a throwaway index and then exercises the *actual*
   queries and mutations EveBox runs during normal operation (the alerts inbox
   aggregation, event histogram, group-by/rare-terms aggregations, DHCP/DNS
   reports, the stats `derivative` pipeline aggregations, `_update_by_query`
   painless tag/comment operations, etc.), reporting pass/fail per operation.
   `evebox test opensearch` is an alias for the same command. It can also run
   read-only against an existing/production datastore (`--existing`) — see
   [Two modes](#two-modes).

2. **`run.sh`** — a harness that starts ES/OpenSearch containers across a
   version matrix and runs `evebox test elastic` against each.

The goal is query/API compatibility, not data correctness: a query that the
server accepts and returns (even with zero results) counts as a pass.

## Event corpus

The test needs newline-delimited EVE JSON. Point it at one or more files or
directories; directories are searched **recursively** for files ending in
`.json` (so rotated files like `eve.1.json-20260623` are skipped). Events are
read round-robin across files so the bounded sample gets a mix of event types.

## Running a single version by hand

```sh
# Start a security-disabled, single-node node:
podman run --rm -d --name os -p 9200:9200 \
    -e discovery.type=single-node \
    -e DISABLE_SECURITY_PLUGIN=true \
    -e DISABLE_INSTALL_DEMO_CONFIG=true \
    opensearchproject/opensearch:2.6.0

# ...or Elasticsearch:
podman run --rm -d --name es -p 9200:9200 \
    -e discovery.type=single-node \
    -e xpack.security.enabled=false \
    docker.elastic.co/elasticsearch/elasticsearch:7.10.2

# Run the test (sample 5000 events):
evebox test elastic -e http://localhost:9200 --limit 5000 ../eve

# Machine-readable output:
evebox test elastic -e http://localhost:9200 --json ../eve
```

## Two modes

**Import mode (default)** imports a sample of events into a *throwaway* index
and runs the full suite, including the data-modifying checks (escalate, archive,
comment, alert-group archive). The index uses a unique per-run prefix
(`evebox-compat-test-<id>`), so its query pattern, mutations, and cleanup can
only ever touch indices created by that run — even if `--index` is pointed at a
real prefix. It is deleted afterwards (use `--keep` to retain it). The base
prefix can be changed with `--index` (default `evebox-compat-test`).

**Existing mode (`--existing`)** runs only read queries against an existing
datastore: it imports nothing, performs no mutations, and deletes nothing, so it
is safe to run against a production cluster. Select the index prefix to query
with `--index` (default `logstash`). The import and data-modifying checks report
`SKIP`.

```sh
# Read-only check against a production datastore (no writes of any kind):
evebox test elastic -e http://localhost:9200 --existing --index logstash
```

Both modes exit non-zero if any check fails.

## Running the matrix

```sh
./run.sh
```

Environment overrides:

| Variable     | Default                          | Meaning                                  |
|--------------|----------------------------------|------------------------------------------|
| `CONTAINER`  | `podman` if present, else `docker` | Container runtime                       |
| `EVE_DIR`    | `<repo>/../eve`                  | Directory of EVE json files to sample    |
| `LIMIT`      | `20000`                          | Max events to import per run             |
| `EVEBOX`     | builds `target/release/evebox`   | Path to the evebox binary                |
| `PORT`       | `9200`                           | Host port to bind                        |
| `KEEP_GOING` | unset                            | Continue after a container fails to start|

Edit the `VERSIONS` array at the top of `run.sh` to change which versions are
tested. Each entry is `engine|version` where engine is `es` or `os`; the image
tags must exist in their registries.

## Notes / limitations

- Only non-ECS mode is tested. EveBox's importer is disabled in ECS mode and we
  have no ECS-shaped corpus; ECS coverage is future work.
- Each container runs with a 1 GB heap and is started one at a time.
- `PULL-FAIL` / `START-FAIL` / `UNHEALTHY` in the summary are container/registry
  problems on the host, not EveBox results. On `UNHEALTHY` the harness prints the
  container's last log lines. Two we hit in practice:
  - **Docker Hub pull failures** (`unable to retrieve auth token`) are usually
    transient rate-limiting; the harness retries pulls. If it persists, run
    `podman login docker.io` (or clear stale credentials).
  - **Elasticsearch 7.17.0** (and other early 7.17.x) bundle a JDK that crashes
    on **cgroup v2** hosts (`CgroupV2Subsystem` NPE) before ES starts — use a
    recent 7.17.x (the matrix uses 7.17.28).
- **Known limitation (reported as `KNWN`, not a failure):** free-text
  (bare-word) search uses a field-less `query_string`, which the engine expands
  across *every* field. On indices with very wide mappings (Suricata EVE easily
  exceeds 1024 fields) Elasticsearch 7.x and OpenSearch reject this with
  `field expansion for [*] matches too many fields`. This is the same on both
  engines (not a compatibility difference) and does not occur where the
  field-expansion limit is high enough (e.g. Elasticsearch 8.x) or on narrower
  mappings. Field-qualified searches (`field:value`) are unaffected.
