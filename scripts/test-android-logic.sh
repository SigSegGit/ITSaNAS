#!/usr/bin/env bash
# Runs the Android client's API-contract tests (android/logic-tests) — a
# standalone, non-Android Gradle project that compiles the real
# network/*.kt production sources (not copies) and pins their JSON wire
# shape against itsanas-daemon's actual field names.
#
# Deliberately isolated from android/app's Gradle build: that one needs
# the Android Gradle Plugin (Google's Maven repo), which isn't available
# in every environment (see android/README.md); this one only needs Maven
# Central, so it can run anywhere with `gradle` and a JVM.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../android/logic-tests"

gradle test --console=plain
