#!/bin/bash
# Mugen Launcher wrapper — handles Steam Runtime compatibility

# Fix libcups missing from Steam Runtime (Electron dependency)
CUPS_LIB=$(find /usr/lib /usr/lib64 /usr/lib32 -name "libcups.so.2" 2>/dev/null | head -1)
if [ -n "$CUPS_LIB" ]; then
    export LD_LIBRARY_PATH="$(dirname "$CUPS_LIB"):${LD_LIBRARY_PATH:-}"
fi

exec ~/.local/share/mugen/launcher/Mugen.AppImage \
    --no-sandbox \
    --ozone-platform=x11 \
    --disable-gpu-compositing \
    --in-process-gpu \
    --disable-gpu-sandbox \
    "$@"
