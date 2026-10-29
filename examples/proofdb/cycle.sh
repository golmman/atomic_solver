#!/usr/bin/env bash

while true; do
    echo "$(date '+%Y-%m-%dT%H:%M:%S') - start new harvest cycle"

    make proofdb_cycle_run &
    run_pid=$!

    # After an hour, request a graceful stop if the run is still active.
    (
        sleep 3600
        if kill -0 "$run_pid" 2>/dev/null; then
            make proofdb_cycle_stop
        fi
    ) &
    timer_pid=$!

    wait "$run_pid"
    run_status=$?

    # Cancel the timer if the run finished before the hour elapsed.
    kill "$timer_pid" 2>/dev/null || true
    wait "$timer_pid" 2>/dev/null || true

    if (( run_status != 0 )); then
        echo "proofdb_cycle_run failed; stopping." >&2
        exit "$run_status"
    fi

    make proofdb_sync || exit $?
done
