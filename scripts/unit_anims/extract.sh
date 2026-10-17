#!/bin/sh
# Regenerates ../../src/units_table.rs from Outpost2.exe (needs objdump and python3).
# Method: see the module docs of src/units.rs. Usage: extract.sh [path/to/Outpost2.exe]
set -eu
here=$(cd "$(dirname "$0")" && pwd)
root="$here/../../../../.."
export OP2_EXE="${1:-$root/prefix/drive_c/GOG Games/Outpost 2/Outpost2.exe}"
export TETHYS_MAPOBJECTTYPE_H="$root/vendor/TethysAPI/Game/MapObjectType.h"
tmp=$(mktemp -d)
python3 -I "$here/tables.py" "$here" "$tmp/tables.json" > /dev/null
python3 -I "$here/turrets.py" "$here" "$tmp/turrets.json" > /dev/null
python3 -I "$here/gen.py" "$tmp/tables.json" "$tmp/turrets.json" > "$here/../../src/units_table.rs"
rm -r "$tmp"
echo "wrote src/units_table.rs"
