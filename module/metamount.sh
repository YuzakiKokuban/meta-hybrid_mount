MODDIR="${0%/*}"
BASE_DIR="/data/adb/meta-hybrid"
IMG_FILE="$MODDIR/modules.img"
MNT_DIR="$MODDIR/rw"
LOG_FILE="$BASE_DIR/daemon.log"
mkdir -p "$BASE_DIR"
if [ -f "$LOG_FILE" ]; then
    rm "$LOG_FILE"
fi
log() {
    echo "[Wrapper] $1" >> "$LOG_FILE"
}
log "Starting Hybrid Mount..."
BINARY="$MODDIR/meta-hybrid"
if ! mountpoint -q "$MNT_DIR" 2>/dev/null; then
    log "Image not mounted, mounting now..."

    # Check if image file exists
    if [ ! -f "$IMG_FILE" ]; then
        log "ERROR: Image file not found at $IMG_FILE"
        exit 1
    fi

    # Create mount point
    mkdir -p "$MNT_DIR"

    # Mount the ext4 image
    chcon u:object_r:ksu_file:s0 "$IMG_FILE" 2>/dev/null
    mount -t ext4 -o loop,rw,noatime "$IMG_FILE" "$MNT_DIR" || {
        log "ERROR: Failed to mount image"
        exit 1
    }
    log "Image mounted successfully at $MNT_DIR"
else
    log "Image already mounted at $MNT_DIR"
fi

if [ ! -f "$BINARY" ]; then
    log "ERROR: Binary not found at $BINARY"
    exit 1
fi

if [ -f "/data/adb/hybrid_mount/daemon.log" ]; then
  mv "/data/adb/hybrid_mount/daemon.log" "/data/adb/hybrid_mount/daemon.log.bak"
fi

chmod 755 "$BINARY"
"$BINARY" >> "$LOG_FILE" 2>&1
EXIT_CODE=$?
log "Hybrid Mount exited with code $EXIT_CODE"
if [ "$EXIT_CODE" = "0" ]; then
    /data/adb/ksud kernel notify-module-mounted
fi
exit $EXIT_CODE
