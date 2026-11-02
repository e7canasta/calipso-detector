# A fixture that check-guardians.sh does not run must fail the gate: this one adds a fixture the list does not name to the copy, and check.sh must notice it.
cat > scripts/fixtures/unlisted.sh <<'RS'
# Nothing runs this file.
RS
