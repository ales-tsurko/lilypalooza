# LilyPond Error Path Scripts

These scripts launch `Lilypalooza` with controlled `PATH` changes to force specific
startup check failures and verify the error prompt UI.

Run them from the repository root:

```bash
./scripts/lilypond-error-tests/01-binary-not-found.sh
./scripts/lilypond-error-tests/02-version-too-old.sh
./scripts/lilypond-error-tests/03-version-command-fails.sh
```
