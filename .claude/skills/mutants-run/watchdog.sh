#!/bin/sh
# Usage: watchdog.sh <container-name>
# Every 5 s, reads the container's memory use and the host's free memory. Kills the container
# when it uses over 3.5 GiB or the host has under 1 GiB free. Prints one line per minute with
# both readings. Exits when the container is gone (after up to 120 s of waiting for it to start).

CONTAINER_LIMIT_MIB=3584
HOST_FLOOR_MIB=1024

# to_mib "<docker MemUsage>": the used amount of "123.4MiB / 4GiB", in whole MiB.
to_mib() {
    echo "$1" | awk '{
        v = $1; unit = v; sub(/^[0-9.]+/, "", unit); sub(/[A-Za-z]+$/, "", v)
        m = 0
        if (unit == "B") m = v / 1048576
        else if (unit == "KiB") m = v / 1024
        else if (unit == "kB" || unit == "KB") m = v / 1048.576
        else if (unit == "MiB") m = v
        else if (unit == "MB") m = v / 1.048576
        else if (unit == "GiB") m = v * 1024
        else if (unit == "GB") m = v * 953.674
        printf "%d\n", m
    }'
}

# host_free_mib: free plus reclaimable memory of the host, in MiB.
host_free_mib() {
    if [ -r /proc/meminfo ]; then
        awk '/^MemAvailable:/ { printf "%d\n", $2 / 1024 }' /proc/meminfo
    else
        vm_stat | awk '
            /page size of/ { for (i = 1; i <= NF; i++) if ($i == "of") size = $(i + 1) }
            /^Pages free:/ { gsub(/\./, "", $3); free = $3 }
            /^Pages inactive:/ { gsub(/\./, "", $3); inactive = $3 }
            END { printf "%d\n", (free + inactive) * size / 1048576 }'
    fi
}

# Lets a test source the functions without starting the loop.
if [ "${WATCHDOG_SOURCE_ONLY:-}" = 1 ]; then return 0 2>/dev/null || exit 0; fi

name=${1:?usage: watchdog.sh <container-name>}
seen=0
waited=0
tick=0

while :; do
    if docker inspect "$name" >/dev/null 2>&1; then
        seen=1
    elif [ "$seen" = 1 ]; then
        echo "watchdog: $name is gone, exiting"
        exit 0
    elif [ "$waited" -ge 120 ]; then
        echo "watchdog: $name never started, exiting"
        exit 0
    else
        waited=$((waited + 5))
        sleep 5
        continue
    fi

    usage=$(docker stats --no-stream --format '{{.MemUsage}}' "$name" 2>/dev/null)
    if [ -n "$usage" ]; then
        used=$(to_mib "$usage")
        free=$(host_free_mib)
        if [ "$used" -gt "$CONTAINER_LIMIT_MIB" ] || [ "$free" -lt "$HOST_FLOOR_MIB" ]; then
            echo "watchdog: KILLING $name: container ${used} MiB (limit ${CONTAINER_LIMIT_MIB}), host free ${free} MiB (floor ${HOST_FLOOR_MIB})"
            docker kill "$name" >/dev/null 2>&1
            exit 0
        fi
        # docker stats itself takes about a second, so count ticks rather than seconds.
        if [ $((tick % 12)) -eq 0 ]; then
            echo "watchdog: $(date +%H:%M:%S) container ${used} MiB, host free ${free} MiB"
        fi
        tick=$((tick + 1))
    fi
    sleep 5
done
