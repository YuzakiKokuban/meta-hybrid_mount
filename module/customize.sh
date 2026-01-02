ui_print "- Extracting module files..."
unzip -o "$ZIPFILE" -d "$MODPATH" >&2
case "$ARCH" in
"arm64")
  ABI="arm64-v8a"
  ;;
"x64")
  ABI="x86_64"
  ;;
"riscv64")
  ABI="riscv64"
  ;;
*)
  abort "! Unsupported architecture: $ARCH"
  ;;
esac
ui_print "- Device Architecture: $ARCH ($ABI)"
BIN_SOURCE="$MODPATH/binaries/$ABI/meta-hybrid"
BIN_TARGET="$MODPATH/meta-hybrid"
if [ ! -f "$BIN_SOURCE" ]; then
  abort "! Binary for $ABI not found in this zip!"
fi
ui_print "- Installing binary for $ABI..."
cp -f "$BIN_SOURCE" "$BIN_TARGET"
set_perm "$BIN_TARGET" 0 0 0755
rm -rf "$MODPATH/binaries"
rm -rf "$MODPATH/system"
BASE_DIR="/data/adb/meta-hybrid"
mkdir -p "$BASE_DIR"
if [ ! -f "$BASE_DIR/config.toml" ]; then
  ui_print "- Installing default config"
  cat "$MODPATH/config.toml" >"$BASE_DIR/config.toml"
fi

IMG_FILE="$MODPATH/modules.img"
IMG_SIZE_MB=2048
EXISTING_IMG="/data/adb/modules/$MODID/modules.img"

if [ -f "$IMG_FILE" ]; then
  ui_print "- Existing modules.img found, keeping it."
  ui_print "- Reusing modules image from previous install"
  "$MODPATH/meta-hybrid" xcp "$EXISTING_IMG" "$IMG_FILE" ||
    abort "! Failed to copy existing modules image"

else
  ui_print "- Creating 2GB ext4 image for module storage"

  # Create sparse file (2GB logical size, 0 bytes actual)
  truncate -s ${IMG_SIZE_MB}M "$IMG_FILE" ||
    abort "! Failed to create image file"

  # Remove journal to prevent creating jbd2 sysfs node
  /system/bin/mke2fs -t ext4 -O ^has_journal -F "$IMG_FILE" >/dev/null 2>&1 ||
    abort "! Failed to format ext4 image"
fi
if [ -z "$KSU" ]; then
  touch /data/adb/.litemode_enable
  chmod 600 /data/adb/.litemode_enable
fi
set_perm_recursive "$MODPATH" 0 0 0755 0644
set_perm "$BIN_TARGET" 0 0 0755
set_perm "$MODPATH/tools/mkfs.erofs" 0 0 0755
ui_print "- Installation complete"
