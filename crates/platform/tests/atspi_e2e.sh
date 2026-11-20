#!/usr/bin/env bash
# Run the AT-SPI end-to-end test on a private session bus.
# Needs: dbus-daemon, at-spi2-core, python3-gi, gir1.2-atspi-2.0 (apt).
set -euo pipefail
cd "$(dirname "$0")/.."
export ALLOY_ATSPI_E2E=1
exec dbus-run-session -- cargo test -p alloy-ui-platform --test atspi -- --nocapture
