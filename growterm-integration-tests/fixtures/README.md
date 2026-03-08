Place raw Codex terminal output fixtures here.

Default fixture path used by the replay test:

- `growterm-integration-tests/fixtures/codex-resume.vt`

You can also point the test at any capture file with:

```sh
GROWTERM_CODEX_VT_FIXTURE=/abs/path/to/codex-resume.vt \
cargo test --manifest-path growterm-integration-tests/Cargo.toml \
  --test codex_resume_vt_replay -- --ignored --nocapture
```
