#!/bin/sh
# A fake test runner for `tests/verify.rs`: reads every tests/*.test.js,
# fails on a line that is exactly FAIL, and names each failing file. When
# VERIFY_MARK is set it appends a line there, so a test can tell this runner
# ran — and not a command the change declared instead.
if [ -n "$VERIFY_MARK" ]; then
  echo "runner.sh ran" >> "$VERIFY_MARK"
fi
status=0
for f in tests/*.test.js; do
  [ -e "$f" ] || continue
  if grep -qx 'FAIL' "$f"; then
    echo "FAIL $f"
    status=1
  else
    echo "PASS $f"
  fi
done
exit $status
